use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};

#[derive(Debug, PartialEq, Clone)]
pub struct View {
    id: i64,
    name: String,
    project_id: i64,
    position: i64,
}

#[derive(Debug)]
pub struct Project {
    active_view_id: i64,
    id: i64,
    name: String,
}

impl Project {
    pub fn name(&self) -> &str {
        &self.name
    }
}

pub struct Repository {
    conn: Connection,
    default_view_name: String,
}

impl Repository {
    pub fn new(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(2))?;
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS views (
                id    INTEGER PRIMARY KEY,
                name  TEXT NOT NULL,
                project_id INTEGER NOT NULL,
                position INTEGER NOT NULL,

                FOREIGN KEY(project_id) REFERENCES projects(id) DEFERRABLE INITIALLY DEFERRED
                UNIQUE(project_id, position)
                UNIQUE(project_id, name)
            );

            CREATE TABLE IF NOT EXISTS projects (
                id    INTEGER PRIMARY KEY,
                name  TEXT NOT NULL UNIQUE,
                active_view_id INTEGER not null,

                FOREIGN KEY(active_view_id) REFERENCES views(id) DEFERRABLE INITIALLY DEFERRED
            );

            CREATE TABLE IF NOT EXISTS pins (
                id    INTEGER PRIMARY KEY,
                key  TEXT NOT NULL UNIQUE,
                view_id INTEGER not null,

                FOREIGN KEY(view_id) REFERENCES views(id)
            );
            "#,
        )?;

        Ok(Self {
            conn: conn,
            default_view_name: "view0".to_string(),
        })
    }

    pub fn create_project(&mut self, name: &str) -> Result<Project> {
        let tx = self.conn.transaction()?;

        // insert the project
        tx.execute(
            "INSERT INTO projects (name, active_view_id) VALUES (?1, ?2)",
            params![name, 0],
        )?;
        let project_id: i64 = tx.last_insert_rowid();

        // insert the view
        tx.execute(
            "INSERT INTO views (name, project_id, position) VALUES (?1, ?2, ?3)",
            params![self.default_view_name, project_id, 0],
        )?;
        let view_id: i64 = tx.last_insert_rowid();

        // update the project to point to the new view
        tx.execute(
            "UPDATE projects SET active_view_id = ?1 WHERE id = ?2",
            params![view_id, project_id],
        )?;

        tx.commit()?;

        self.get_project_by_id(project_id)
            .ok_or(anyhow::anyhow!("project not found"))
    }

    pub fn list_projects(&self) -> Result<Vec<Project>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, active_view_id FROM projects ORDER BY id")?;
        let projects = stmt.query_map([], |row| {
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                active_view_id: row.get(2)?,
            })
        })?;

        projects.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_project_by_id(&self, id: i64) -> Option<Project> {
        self.conn
            .query_row(
                "SELECT id, name, active_view_id FROM projects WHERE id = ?1",
                params![id],
                |row| {
                    Ok(Project {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        active_view_id: row.get(2)?,
                    })
                },
            )
            .optional()
            .ok()?
    }

    pub fn get_project_by_name(&self, name: &str) -> Option<Project> {
        self.conn
            .query_row(
                "SELECT id, name, active_view_id FROM projects WHERE name = ?1",
                params![name],
                |row| {
                    Ok(Project {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        active_view_id: row.get(2)?,
                    })
                },
            )
            .optional()
            .ok()?
    }

    pub fn get_active_view_for_project(&self, project: &Project) -> Option<View> {
        self.conn
            .query_row(
                "SELECT id, name, project_id, position FROM views WHERE id = ?1",
                params![project.active_view_id],
                |row| {
                    Ok(View {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        project_id: row.get(2)?,
                        position: row.get(3)?,
                    })
                },
            )
            .optional()
            .ok()?
    }

    pub fn create_view_in_project(&mut self, project: &Project, name: &str) -> Result<View> {
        let tx = self.conn.transaction()?;

        // get the largest position for a view in the project
        // and increment it by one
        tx.query_row(
            "SELECT MAX(position) FROM views WHERE project_id = ?",
            params![project.id],
            |row| Ok(row.get::<_, i64>(0)?),
        )
        .and_then(|max_position| {
            tx.execute(
                "INSERT INTO views (name, project_id, position) VALUES (?1, ?2, ?3)",
                params![name, project.id, max_position + 1],
            )
        })?;
        let view_id: i64 = tx.last_insert_rowid();

        tx.commit()?;

        let view = self.get_view_by_id(view_id).unwrap();
        Ok(view)
    }

    pub fn set_active_view_for_project(&mut self, project: &Project, view: &View) -> Result<()> {
        if view.project_id != project.id {
            return Err(anyhow::anyhow!("view is not in the project"));
        }

        self.conn.execute(
            "UPDATE projects SET active_view_id = ?1 WHERE id = ?2",
            params![view.id, project.id],
        )?;

        Ok(())
    }

    pub fn get_view_by_id(&self, id: i64) -> Option<View> {
        self.conn
            .query_row(
                "SELECT id, name, project_id, position FROM views WHERE id = ?1",
                params![id],
                |row| {
                    Ok(View {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        project_id: row.get(2)?,
                        position: row.get(3)?,
                    })
                },
            )
            .optional()
            .ok()?
    }

    pub fn list_views(&self) -> Result<Vec<View>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, project_id, position FROM views ORDER BY id")?;
        let views = stmt.query_map([], |row| {
            Ok(View {
                id: row.get(0)?,
                name: row.get(1)?,
                project_id: row.get(2)?,
                position: row.get(3)?,
            })
        })?;

        views.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn get_next_view_for_project(&self, project: &Project) -> Result<View> {
        let active_view = self
            .get_active_view_for_project(project)
            .ok_or(anyhow::anyhow!("no active view"))?;

        let next = self.conn.query_row(
            "SELECT views.id, views.name, views.project_id, views.position FROM views WHERE project_id = ? AND position > ? ORDER BY views.position ASC LIMIT 1",
            params![project.id, active_view.position],
            |row| {
                Ok(View {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    project_id: row.get(2)?,
                    position: row.get(3)?,
                })
            },
        ).optional()?;

        if let Some(next) = next {
            Ok(next)
        } else {
            self.conn.query_row(
                "SELECT views.id, views.name, views.project_id, views.position FROM views WHERE project_id = ? ORDER BY views.position ASC LIMIT 1",
                params![project.id],
                |row| {
                    Ok(View {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        project_id: row.get(2)?,
                        position: row.get(3)?,
                    })
                },
            ).optional()?.ok_or(anyhow::anyhow!("no next view"))
        }
    }

    pub fn get_prev_view_for_project(&self, project: &Project) -> Result<View> {
        let active_view = self
            .get_active_view_for_project(project)
            .ok_or(anyhow::anyhow!("no active view"))?;

        let next = self.conn.query_row(
            "SELECT views.id, views.name, views.project_id, views.position FROM views WHERE project_id = ? AND position < ? ORDER BY views.position DESC LIMIT 1",
            params![project.id, active_view.position],
            |row| {
                Ok(View {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    project_id: row.get(2)?,
                    position: row.get(3)?,
                })
            },
        ).optional()?;

        if let Some(next) = next {
            Ok(next)
        } else {
            self.conn.query_row(
                "SELECT views.id, views.name, views.project_id, views.position FROM views WHERE project_id = ? ORDER BY views.position DESC LIMIT 1",
                params![project.id],
                |row| {
                    Ok(View {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        project_id: row.get(2)?,
                        position: row.get(3)?,
                    })
                },
            ).optional()?.ok_or(anyhow::anyhow!("no next view"))
        }
    }

    pub fn get_window_manager_display_name(&self, view: &View) -> Result<String> {
        let project_name = self.conn.query_row(
            "SELECT name FROM projects WHERE id = ?1",
            params![view.project_id],
            |row| row.get::<_, String>(0),
        );

        if let Some(project_name) = project_name.ok() {
            Ok(format!("{}#{}", project_name, view.name))
        } else {
            Err(anyhow::anyhow!("project not found"))
        }
    }

    fn parse_window_manager_display_name(&self, name: &str) -> Result<(String, String)> {
        let parts = name.split("#").collect::<Vec<&str>>();
        if parts.len() != 2 {
            return Err(anyhow::anyhow!("invalid window manager display name"));
        }
        Ok((parts[0].to_string(), parts[1].to_string()))
    }

    pub fn get_project_from_window_manager_display_name(
        &self,
        name: &str,
    ) -> Result<Option<Project>> {
        let (project_name, _) = self.parse_window_manager_display_name(name)?;

        Ok(self
            .conn
            .query_row(
                "SELECT id, name, active_view_id FROM projects WHERE name = ?1",
                params![project_name],
                |row| {
                    Ok(Project {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        active_view_id: row.get(2)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn get_view_from_window_manager_display_name(&self, name: &str) -> Result<Option<View>> {
        let (project_name, view_name) = self.parse_window_manager_display_name(name)?;

        Ok(self.conn.query_row(
            "SELECT views.id, views.name, views.project_id, views.position FROM projects JOIN views ON projects.id = views.project_id WHERE projects.name = ?1 and views.name = ?2",
            params![project_name, view_name],
            |row| {
                Ok(View {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        project_id: row.get(2)?,
                        position: row.get(3)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn upsert_pin(&mut self, key: &str, view: &View) -> Result<()> {
        self.conn.execute(
            "INSERT INTO pins (key, view_id) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET view_id = ?2",
            params![key, view.id],

        )?;
        Ok(())
    }

    pub fn clear_pin(&mut self, key: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM pins WHERE key = ?1", params![key])
            .optional()?;
        Ok(())
    }

    pub fn get_view_for_pin_key(&self, key: &str) -> Option<View> {
        self.conn
            .query_row(
            "SELECT views.id, views.name, views.project_id, views.position FROM views JOIN pins ON views.id = pins.view_id WHERE pins.key = ?1",
            params![key],
            |row| {
                Ok(View {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        project_id: row.get(2)?,
                        position: row.get(3)?,
                    })
                },
            )
            .optional()
            .ok()?
    }

    pub fn get_pin_key_for_view(&self, view: &View) -> Option<String> {
        self.conn
            .query_row(
                "SELECT key FROM pins WHERE view_id = ?1",
                params![view.id],
                |row| Ok(row.get(0)?),
            )
            .optional()
            .ok()?
    }
}

#[cfg(test)]
mod tests {
    use super::Project;
    use super::Repository;
    use super::View;
    use rusqlite::Connection;

    #[test]
    fn test_create_project_creates_project_and_active_view() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let project = repo.create_project("proj1").unwrap();
        assert_eq!(project.name, "proj1");

        let view = repo.get_active_view_for_project(&project).unwrap();
        assert_eq!(view.name, "view0");
        assert_eq!(view.id, project.active_view_id);
    }

    #[test]
    fn test_create_project_fails_if_name_already_exists() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let _ = repo.create_project("proj1").unwrap();
        assert!(repo.create_project("proj1").is_err());
    }

    #[test]
    fn test_list_projects_when_there_are_projects() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let proj1 = repo.create_project("proj1").unwrap();
        let proj2 = repo.create_project("proj2").unwrap();
        // in the check portion of the test, we "implicitly"
        // assume that the projects are returned by id in ascending order
        // so confirm that on creation.
        assert!(proj1.id < proj2.id);

        let projects = repo.list_projects().unwrap();
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].name, "proj1");
        assert_eq!(projects[1].name, "proj2");
    }

    #[test]
    fn test_list_projects_when_there_are_no_projects() {
        let conn = Connection::open_in_memory().unwrap();
        let repo = Repository::new(conn).unwrap();

        let projects = repo.list_projects();
        assert!(projects.is_ok());
        assert_eq!(projects.unwrap().len(), 0);
    }

    #[test]
    fn test_get_project_by_id_when_projects_is_found() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let proj1 = repo.create_project("proj1").unwrap();
        let proj2 = repo.create_project("proj2").unwrap();

        let project = repo.get_project_by_id(proj1.id).unwrap();
        assert_eq!(project.name, "proj1");

        let project = repo.get_project_by_id(proj2.id).unwrap();
        assert_eq!(project.name, "proj2");
    }

    #[test]
    fn test_get_project_by_id_when_projects_is_not_found() {
        let conn = Connection::open_in_memory().unwrap();
        let repo = Repository::new(conn).unwrap();

        // we expect that id 1 is not found because we just created in memory db
        // a few lines above
        let project = repo.get_project_by_id(1);
        assert!(project.is_none());
    }

    #[test]
    fn test_get_project_by_name_when_projects_is_found() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let proj1 = repo.create_project("proj1").unwrap();
        let proj2 = repo.create_project("proj2").unwrap();

        let project = repo.get_project_by_name(proj1.name()).unwrap();
        assert_eq!(project.name, proj1.name);

        let project = repo.get_project_by_name(proj2.name()).unwrap();
        assert_eq!(project.name, proj2.name);
    }

    #[test]
    fn test_get_project_by_name_when_projects_is_not_found() {
        let conn = Connection::open_in_memory().unwrap();
        let repo = Repository::new(conn).unwrap();

        let project = repo.get_project_by_name("not-found");
        assert!(project.is_none());
    }

    #[test]
    fn test_get_active_view_for_project_when_project_is_found() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let proj1 = repo.create_project("proj1").unwrap();
        let _ = repo.create_project("proj2").unwrap();

        let proj1_active_view = repo.get_active_view_for_project(&proj1).unwrap();
        assert_eq!(proj1_active_view.name, "view0");
        assert_eq!(proj1_active_view.id, proj1.active_view_id);
    }

    #[test]
    fn test_get_active_view_for_project_when_project_is_not_found() {
        let conn = Connection::open_in_memory().unwrap();
        let repo = Repository::new(conn).unwrap();

        let project = repo.get_active_view_for_project(&Project {
            id: 1,
            name: "proj1".to_string(),
            active_view_id: 1,
        });
        assert!(project.is_none());
    }

    #[test]
    fn test_crate_view_in_project_when_project_is_found() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let proj1 = repo.create_project("proj1").unwrap();
        let view = repo.create_view_in_project(&proj1, "view2").unwrap();
        assert_eq!(view.name, "view2");
        assert_eq!(view.project_id, proj1.id);
        assert_eq!(view.position, 1);
    }

    #[test]
    fn test_create_view_in_project_when_project_is_not_found() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let proj = Project {
            id: 1,
            name: "proj1".to_string(),
            active_view_id: 1,
        };

        let view = repo.create_view_in_project(&proj, "view");
        assert!(view.is_err());
    }

    #[test]
    fn test_create_view_in_project_when_project_is_found_but_view_name_already_exists() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let proj1 = repo.create_project("proj1").unwrap();
        let proj1_active_view = repo.get_active_view_for_project(&proj1).unwrap();

        let view = repo.create_view_in_project(&proj1, proj1_active_view.name.as_str());
        assert!(view.is_err());
    }

    #[test]
    fn test_set_active_view_for_project_when_view_is_found() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let proj1 = repo.create_project("proj1").unwrap();
        let proj1_extra_view = repo.create_view_in_project(&proj1, "view1").unwrap();

        let proj1_active_view = repo.get_active_view_for_project(&proj1).unwrap();
        assert_ne!(proj1_active_view, proj1_extra_view);

        repo.set_active_view_for_project(&proj1, &proj1_extra_view)
            .unwrap();
        let updated_proj1 = repo.get_project_by_id(proj1.id).unwrap();

        let proj1_active_view = repo.get_active_view_for_project(&updated_proj1).unwrap();
        assert_eq!(proj1_active_view, proj1_extra_view);
    }

    #[test]
    fn test_set_active_view_for_project_when_view_is_not_found() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let proj1 = repo.create_project("proj1").unwrap();

        // make sure that there is no view with id 2
        let view_id = 2;
        assert!(repo.get_view_by_id(view_id).is_none());

        let view = View {
            id: view_id,
            name: "view1".to_string(),
            project_id: proj1.id,
            position: 1,
        };

        let view = repo.set_active_view_for_project(&proj1, &view);
        assert!(view.is_err());
    }

    #[test]
    fn test_set_active_view_for_project_when_view_is_not_in_project() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let proj1 = repo.create_project("proj1").unwrap();
        let proj1_view = repo.create_view_in_project(&proj1, "view1").unwrap();

        let proj2 = repo.create_project("proj2").unwrap();

        let view = repo.set_active_view_for_project(&proj2, &proj1_view);
        assert!(view.is_err());
    }

    #[test]
    fn test_get_view_by_id_when_view_is_found() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let proj1 = repo.create_project("proj1").unwrap();
        let view = repo.get_active_view_for_project(&proj1).unwrap();

        let retrieved_view = repo.get_view_by_id(view.id).unwrap();
        assert_eq!(retrieved_view, view);
    }

    #[test]
    fn test_get_view_by_id_when_view_is_not_found() {
        let conn = Connection::open_in_memory().unwrap();
        let repo = Repository::new(conn).unwrap();

        let view = repo.get_view_by_id(1);
        assert!(view.is_none());
    }

    #[test]
    fn test_list_views_when_views_are_found() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let proj1 = repo.create_project("proj1").unwrap();
        let proj1_view0 = repo.get_active_view_for_project(&proj1).unwrap();
        let proj1_view1 = repo.create_view_in_project(&proj1, "view1").unwrap();
        let proj1_view2 = repo.create_view_in_project(&proj1, "view2").unwrap();

        let views = repo.list_views().unwrap();
        assert_eq!(views.len(), 3);
        assert_eq!(views[0], proj1_view0);
        assert_eq!(views[1], proj1_view1);
        assert_eq!(views[2], proj1_view2);
    }

    #[test]
    fn test_list_views_when_views_are_not_found() {
        let conn = Connection::open_in_memory().unwrap();
        let repo = Repository::new(conn).unwrap();

        let views = repo.list_views();
        assert!(views.is_ok());
        assert_eq!(views.unwrap().len(), 0);
    }

    #[test]
    fn test_get_next_view_for_project_when_views_are_found() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let proj1 = repo.create_project("proj1").unwrap();
        let proj1_view0 = repo.get_active_view_for_project(&proj1).unwrap();
        let proj1_view1 = repo.create_view_in_project(&proj1, "view1").unwrap();
        let proj1_view2 = repo.create_view_in_project(&proj1, "view2").unwrap();

        let next_view = repo.get_next_view_for_project(&proj1).unwrap();
        assert_eq!(next_view, proj1_view1);

        // update the active view and then reload the project since we just changed it
        assert!(repo.set_active_view_for_project(&proj1, &next_view).is_ok());
        let updated_proj1 = repo.get_project_by_id(proj1.id).unwrap();
        let next_view = repo.get_next_view_for_project(&updated_proj1).unwrap();
        assert_eq!(next_view, proj1_view2);

        // one more time to make sure we cycle back to the first view
        assert!(repo.set_active_view_for_project(&proj1, &next_view).is_ok());
        let updated_proj1 = repo.get_project_by_id(proj1.id).unwrap();
        let next_view = repo.get_next_view_for_project(&updated_proj1).unwrap();
        assert_eq!(next_view, proj1_view0);
    }

    #[test]
    fn test_get_next_view_for_project_when_project_is_not_found() {
        let conn = Connection::open_in_memory().unwrap();
        let repo = Repository::new(conn).unwrap();

        let project = repo.get_next_view_for_project(&Project {
            id: 1,
            name: "proj1".to_string(),
            active_view_id: 1,
        });
        assert!(project.is_err());
    }

    #[test]
    fn test_get_prev_view_for_project_when_views_are_found() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let proj1 = repo.create_project("proj1").unwrap();
        let proj1_view0 = repo.get_active_view_for_project(&proj1).unwrap();
        let proj1_view1 = repo.create_view_in_project(&proj1, "view1").unwrap();
        let proj1_view2 = repo.create_view_in_project(&proj1, "view2").unwrap();

        let prev_view = repo.get_prev_view_for_project(&proj1).unwrap();
        assert_eq!(prev_view, proj1_view2);

        // update the active view and then reload the project since we just changed it
        assert!(repo.set_active_view_for_project(&proj1, &prev_view).is_ok());
        let updated_proj1 = repo.get_project_by_id(proj1.id).unwrap();
        let prev_view = repo.get_prev_view_for_project(&updated_proj1).unwrap();
        assert_eq!(prev_view, proj1_view1);

        // one more time to make sure we cycle back to the last view
        assert!(repo.set_active_view_for_project(&proj1, &prev_view).is_ok());
        let updated_proj1 = repo.get_project_by_id(proj1.id).unwrap();
        let prev_view = repo.get_prev_view_for_project(&updated_proj1).unwrap();
        assert_eq!(prev_view, proj1_view0);
    }

    #[test]
    fn test_get_prev_view_for_project_with_project_is_not_found() {
        let conn = Connection::open_in_memory().unwrap();
        let repo = Repository::new(conn).unwrap();

        let project = repo.get_prev_view_for_project(&Project {
            id: 1,
            name: "proj1".to_string(),
            active_view_id: 1,
        });
        assert!(project.is_err());
    }

    #[test]
    fn test_get_window_manager_display_name_when_view_is_found() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let proj1 = repo.create_project("proj1").unwrap();
        let proj1_view = repo.get_active_view_for_project(&proj1).unwrap();

        let window_manager_id = repo.get_window_manager_display_name(&proj1_view).unwrap();
        assert_eq!(window_manager_id, "proj1#view0");
    }

    #[test]
    fn test_get_window_manager_display_name_when_view_is_not_found() {
        let conn = Connection::open_in_memory().unwrap();
        let repo = Repository::new(conn).unwrap();

        let view = repo.get_window_manager_display_name(&View {
            id: 1,
            name: "view1".to_string(),
            project_id: 1,
            position: 1,
        });
        assert!(view.is_err());
    }

    #[test]
    fn test_get_project_from_window_manager_display_name_when_project_is_found() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let proj1 = repo.create_project("proj1").unwrap();
        let proj1_view = repo.get_active_view_for_project(&proj1).unwrap();
        let wm_display_name = repo.get_window_manager_display_name(&proj1_view).unwrap();

        let retreived_project = repo
            .get_project_from_window_manager_display_name(wm_display_name.as_str())
            .unwrap()
            .unwrap();
        assert_eq!(retreived_project.id, proj1.id);
    }

    #[test]
    fn test_get_project_from_window_manager_display_name_when_project_is_not_found() {
        let conn = Connection::open_in_memory().unwrap();
        let repo = Repository::new(conn).unwrap();

        let retreived_project = repo
            .get_project_from_window_manager_display_name("not-found#view")
            .unwrap();
        assert!(retreived_project.is_none());
    }

    #[test]
    fn test_get_project_from_window_manager_display_name_when_name_is_invalid() {
        let conn = Connection::open_in_memory().unwrap();
        let repo = Repository::new(conn).unwrap();

        let retreived_project = repo.get_project_from_window_manager_display_name("invalid-name");
        assert!(retreived_project.is_err());
    }

    #[test]
    fn test_get_view_from_window_manager_display_name_when_view_is_found() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let proj1 = repo.create_project("proj1").unwrap();
        let view = repo.get_active_view_for_project(&proj1).unwrap();

        let window_manager_id = repo.get_window_manager_display_name(&view).unwrap();
        assert_eq!(window_manager_id, "proj1#view0");

        let view = repo
            .get_view_from_window_manager_display_name("proj1#view0")
            .unwrap()
            .unwrap();
        assert_eq!(view.id, view.id);
    }

    #[test]
    fn test_get_view_from_window_manager_display_name_when_view_is_not_found() {
        let conn = Connection::open_in_memory().unwrap();
        let repo = Repository::new(conn).unwrap();

        let view = repo
            .get_view_from_window_manager_display_name("not-found#view")
            .unwrap();
        assert!(view.is_none());
    }

    #[test]
    fn test_get_view_from_window_manager_display_name_when_name_is_invalid() {
        let conn = Connection::open_in_memory().unwrap();
        let repo = Repository::new(conn).unwrap();

        let view = repo.get_view_from_window_manager_display_name("invalid-name");
        assert!(view.is_err());
    }

    #[test]
    fn test_upsert_pin_when_pin_unused_and_view_is_found() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let proj1 = repo.create_project("proj1").unwrap();
        let view = repo.get_active_view_for_project(&proj1).unwrap();

        let key = "g";
        assert!(repo.upsert_pin(key, &view).is_ok());

        // make sure the pin was inserted
        let pinned_view = repo.get_view_for_pin_key(key).unwrap();
        assert_eq!(pinned_view.id, view.id);
    }

    #[test]
    fn test_upsert_pin_when_pin_is_used_and_view_is_found() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let key = "g";

        // set initial pin
        let proj1 = repo.create_project("proj1").unwrap();
        let proj1_view = repo.get_active_view_for_project(&proj1).unwrap();
        assert!(repo.upsert_pin(key, &proj1_view).is_ok());

        // make sure the pin was inserted correctly
        let pinned_view = repo.get_view_for_pin_key(key).unwrap();
        assert_eq!(pinned_view.id, proj1_view.id);

        // update the pin to the new view
        let proj2 = repo.create_project("proj2").unwrap();
        let proj2_view = repo.get_active_view_for_project(&proj2).unwrap();
        assert!(repo.upsert_pin(key, &proj2_view).is_ok());

        // make sure the pin was updated
        let pinned_view = repo.get_view_for_pin_key(key).unwrap();
        assert_eq!(pinned_view.id, proj2_view.id);
    }

    #[test]
    fn test_upsert_pin_when_pin_is_used_and_view_is_not_found() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let non_existent_view = View {
            id: 1,
            name: "view1".to_string(),
            project_id: 1,
            position: 1,
        };

        let key = "g";
        assert!(repo.upsert_pin(key, &non_existent_view).is_err());
    }

    #[test]
    fn test_upsert_pin_when_pin_is_not_used_and_view_is_not_found() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let key = "g";

        // set initial pin
        let proj1 = repo.create_project("proj1").unwrap();
        let proj1_view = repo.get_active_view_for_project(&proj1).unwrap();
        assert!(repo.upsert_pin(key, &proj1_view).is_ok());

        let view_id = 10;
        assert!(repo.get_view_by_id(view_id).is_none());

        let non_existent_view = View {
            id: view_id,
            name: "view1".to_string(),
            project_id: 1,
            position: 1,
        };

        let key = "g";
        assert!(repo.upsert_pin(key, &non_existent_view).is_err());
    }

    #[test]
    fn test_clear_pin_when_pin_is_found() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let key = "g";

        // set pin
        let proj1 = repo.create_project("proj1").unwrap();
        let proj1_view = repo.get_active_view_for_project(&proj1).unwrap();
        assert!(repo.upsert_pin(key, &proj1_view).is_ok());

        // clear the pin
        assert!(repo.clear_pin(key).is_ok());

        // make sure the pin was cleared
        let pinned_view = repo.get_view_for_pin_key(key);
        assert!(pinned_view.is_none());
    }

    #[test]
    fn test_clear_pin_when_pin_is_not_found() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let key = "g";
        assert!(repo.clear_pin(key).is_ok());
    }

    #[test]
    fn test_get_view_for_pin_key_when_pin_is_found() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let pin_key = "g";
        let project = repo.create_project("proj1").unwrap();
        let view = repo.get_active_view_for_project(&project).unwrap();
        repo.upsert_pin(pin_key, &view).unwrap();

        let retrieved_view = repo.get_view_for_pin_key(pin_key).unwrap();
        assert_eq!(retrieved_view, view);
    }

    #[test]
    fn test_get_view_for_pin_key_when_pin_is_not_found() {
        let conn = Connection::open_in_memory().unwrap();
        let repo = Repository::new(conn).unwrap();

        let view = repo.get_view_for_pin_key("g");
        assert!(view.is_none());
    }

    #[test]
    fn test_get_pin_key_for_view_when_pin_is_found() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let pin_key = "g";
        let project = repo.create_project("proj1").unwrap();
        let view = repo.get_active_view_for_project(&project).unwrap();
        repo.upsert_pin(pin_key, &view).unwrap();

        let retrieved_pin_key = repo.get_pin_key_for_view(&view).unwrap();
        assert_eq!(retrieved_pin_key, pin_key);
    }

    #[test]
    fn test_get_pin_key_for_view_when_pin_is_not_found() {
        let conn = Connection::open_in_memory().unwrap();
        let repo = Repository::new(conn).unwrap();

        let view = View {
            id: 1,
            name: "view1".to_string(),
            project_id: 1,
            position: 1,
        };

        let pin_key = repo.get_pin_key_for_view(&view);
        assert!(pin_key.is_none());
    }
}
