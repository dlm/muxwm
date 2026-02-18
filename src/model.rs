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
            default_view_name: "view".to_string(),
        })
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

    pub fn add_project(&mut self, name: &str) -> Result<i64> {
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

        Ok(project_id)
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

    pub fn add_view_to_project(&mut self, project: &Project, name: &str) -> Result<View> {
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
        let _ = self.set_active_view_for_project(project, &view);
        Ok(view)
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

    pub fn set_active_view_for_project(&mut self, project: &Project, view: &View) -> Result<View> {
        if view.project_id != project.id {
            return Err(anyhow::anyhow!("view is not in the project"));
        }

        self.conn.execute(
            "UPDATE projects SET active_view_id = ?1 WHERE id = ?2",
            params![view.id, project.id],
        )?;
        Ok(view.clone())
    }

    pub fn get_window_manager_display_name(&self, view: &View) -> Option<String> {
        let project_name = self
            .conn
            .query_row(
                "SELECT name FROM projects WHERE id = ?1",
                params![view.project_id],
                |row| row.get::<_, String>(0),
            )
            .ok()?;
        Some(format!("{}#{}", project_name, view.name))
    }

    pub fn get_project_from_window_manager_display_name(&self, name: &str) -> Option<Project> {
        let parts = name.split("#").collect::<Vec<&str>>();
        if parts.len() != 2 {
            return None;
        }

        let project_name = parts[0];
        self.conn
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
            .optional()
            .ok()?
    }

    pub fn get_view_from_window_manager_display_name(&self, name: &str) -> Option<View> {
        let parts = name.split("#").collect::<Vec<&str>>();
        if parts.len() != 2 {
            return None;
        }

        let project_name = parts[0];
        let view_name = parts[1];
        self.conn.query_row(
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
            .optional()
            .ok()?
    }

    pub fn upsert_pin(&mut self, key: &str, view: &View) -> Result<i64> {
        // upsert the pin
        self.conn.execute(
            "INSERT INTO pins (key, view_id) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET view_id = ?2",
            params![key, view.id],
        )?;
        let pin_id: i64 = self.conn.last_insert_rowid();

        Ok(pin_id)
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
    use super::Repository;
    use rusqlite::Connection;

    #[test]
    fn playground() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();

        let r = repo.add_project("proj1").unwrap();
        let project = repo.get_project_by_id(r).unwrap();
        let active_view = repo.get_active_view_for_project(&project).unwrap();
        // let mut window_manager_id = repo.get_window_manager_display_name(&active_view).unwrap();
        // assert_eq!(window_manager_id, "proj1#view");
        // repo.upsert_pin("g", &active_view).unwrap();
        // let goto_view = repo.get_view_for_pin_key("g").unwrap();
        // window_manager_id = repo.get_window_manager_display_name(&goto_view).unwrap();
        // assert_eq!(window_manager_id, "proj1#view");

        let view = repo
            .get_view_from_window_manager_display_name("proj1#view")
            .unwrap();
        assert_eq!(view.id, active_view.id);

        // create a second project
        repo.add_project("proj2").unwrap();

        // and list them
        let projects = repo.list_projects().unwrap();
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].name, "proj1");
        assert_eq!(projects[1].name, "proj2");
    }
}
