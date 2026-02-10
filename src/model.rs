use std::collections::HashMap;

use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};

#[derive(Debug, PartialEq, Clone)]
pub struct View {
    id: i64,
    name: String,
    project_id: i64,
}

#[derive(Debug)]
pub struct Project {
    active_view_id: i64,
    id: i64,
    name: String,
}

pub struct Repository {
    conn: Connection,
    default_view_name: String,
    pins: HashMap<String, String>,
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
            "#,
        )?;

        Ok(Self {
            conn: conn,
            default_view_name: "view".to_string(),
            pins: HashMap::from([
                // pre-defined pins
                ("g".to_string(), "admin#view".to_string()),
                ("f".to_string(), "dev#view".to_string()),
                ("d".to_string(), "ref#view".to_string()),
                ("s".to_string(), "ai#view".to_string()),
                ("a".to_string(), "chat#view".to_string()),
                ("`".to_string(), "share#view".to_string()),
                // user-defined pins
                ("0".to_string(), "0.open".to_string()),
                ("1".to_string(), "1.open".to_string()),
                ("2".to_string(), "2.open".to_string()),
                ("3".to_string(), "3.open".to_string()),
                ("4".to_string(), "4.open".to_string()),
                ("5".to_string(), "5.open".to_string()),
                ("6".to_string(), "6.open".to_string()),
                ("7".to_string(), "7.open".to_string()),
                ("8".to_string(), "8.open".to_string()),
                ("9".to_string(), "9.open".to_string()),
            ]),
        })
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
                "SELECT id, name, project_id FROM views WHERE id = ?1",
                params![project.active_view_id],
                |row| {
                    Ok(View {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        project_id: row.get(2)?,
                    })
                },
            )
            .optional()
            .ok()?
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

    pub fn get_view_for_pin_key(&self, key: &str) -> Option<View> {
        // this is a pretty big hack but should improve once we implement
        // the pin table
        let display_name_hack = self.pins.get(key)?;
        let parts = display_name_hack.split("#").collect::<Vec<&str>>();
        if parts.len() != 2 {
            return None;
        }

        let project_name = parts[0];
        let view_name = parts[1];
        let project = self.get_project_by_name(project_name)?;
        let view = self.get_active_view_for_project(&project)?;
        if view.name != view_name {
            return None;
        }
        Some(view)
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
        let r = repo.add_project("Project1").unwrap();
        repo.add_project("admin").unwrap();
        repo.add_project("dev").unwrap();
        repo.add_project("ref").unwrap();
        repo.add_project("ai").unwrap();
        repo.add_project("chat").unwrap();
        repo.add_project("share").unwrap();

        let project = repo.get_project_by_id(r).unwrap();
        assert_eq!(project.name, "Project1");

        let active_view = repo.get_active_view_for_project(&project).unwrap();
        assert_eq!(active_view.name, "view");

        let mut window_manager_id = repo.get_window_manager_display_name(&active_view).unwrap();
        assert_eq!(window_manager_id, "Project1#view");

        let mut goto_view = repo.get_view_for_pin_key("a").unwrap();
        window_manager_id = repo.get_window_manager_display_name(&goto_view).unwrap();
        assert_eq!(window_manager_id, "chat#view");

        goto_view = repo.get_view_for_pin_key("s").unwrap();
        window_manager_id = repo.get_window_manager_display_name(&goto_view).unwrap();
        assert_eq!(window_manager_id, "ai#view");

        goto_view = repo.get_view_for_pin_key("d").unwrap();
        window_manager_id = repo.get_window_manager_display_name(&goto_view).unwrap();
        assert_eq!(window_manager_id, "ref#view");

        goto_view = repo.get_view_for_pin_key("f").unwrap();
        window_manager_id = repo.get_window_manager_display_name(&goto_view).unwrap();
        assert_eq!(window_manager_id, "dev#view");

        goto_view = repo.get_view_for_pin_key("g").unwrap();
        window_manager_id = repo.get_window_manager_display_name(&goto_view).unwrap();
        assert_eq!(window_manager_id, "admin#view");

        goto_view = repo.get_view_for_pin_key("`").unwrap();
        window_manager_id = repo.get_window_manager_display_name(&goto_view).unwrap();
        assert_eq!(window_manager_id, "share#view");
    }
}
