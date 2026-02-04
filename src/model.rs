use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, Transaction, params};

#[derive(Debug, PartialEq, Clone)]
pub struct View {
    id: i64,
    name: String,
    project_id: i64,
}

#[derive(Debug)]
pub struct Project {
    id: i64,
    name: String,
    active_view_id: i64,
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
            "#,
        )?;

        Ok(Self {
            conn: conn,
            default_view_name: "view".to_string(),
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

    pub fn get_window_manager_id(&self, view: &View) -> Option<String> {
        let project_name = self
            .conn
            .query_row(
                "SELECT name FROM projects WHERE id = ?1",
                params![view.project_id],
                |row| row.get::<_, String>(0),
            )
            .ok()?;
        Some(format!("{}#{}#{}", view.id, project_name, view.name))
    }

    // pub fn add_project(&mut self, project: Project) {
    //     self.projects.push(project);
    // }
    //
    // pub fn find_view(&self, project: &str, tag: &str) -> Option<&View> {
    //     self.find_project(project).and_then(|p| p.find_view(tag))
    // }
    //
    // pub fn find_project(&self, project: &str) -> Option<&Project> {
    //     self.projects.iter().find(|p| p.name == project)
    // }
    //
    // pub fn set_active_project(&mut self, project: Project) {
    //     self.active = Some(project);
    // }
}

// fn main() -> Result<()> {
//     // let conn = Connection::open("app.db")?;
//     let conn = Connection::open_in_memory()?;
//
//     conn.pragma_update(None, "foreign_keys", "ON")?;
//     // Optional, but fine even for single-writer:
//     conn.busy_timeout(std::time::Duration::from_secs(2))?;
//
//     conn.execute_batch(
//         r#"
//         CREATE TABLE IF NOT EXISTS tasks (
//             id    INTEGER PRIMARY KEY,
//             title TEXT NOT NULL UNIQUE,
//             done  INTEGER NOT NULL DEFAULT 0
//         );
//         "#,
//     )?;
//
//     let id = insert_task(&conn, "ship v0")?;
//     println!("Inserted task id={id}");
//
//     mark_done(&conn, "ship v0")?;
//
//     let maybe = get_task(&conn, "ship v0")?;
//     println!("Task: {maybe:#?}");
//
//     Ok(())
// }

// fn insert_task(conn: &Connection, title: &str) -> Result<i64> {
//     conn.execute(
//         "INSERT OR IGNORE INTO tasks (title, done) VALUES (?1, 0)",
//         params![title],
//     )?;
//
//     // Return the id whether it was newly inserted or already existed
//     let id: i64 = conn.query_row(
//         "SELECT id FROM tasks WHERE title = ?1",
//         params![title],
//         |row| row.get(0),
//     )?;
//     Ok(id)
// }
//
// fn mark_done(conn: &Connection, title: &str) -> Result<()> {
//     conn.execute("UPDATE tasks SET done = 1 WHERE title = ?1", params![title])?;
//     Ok(())
// }
//
// fn get_task(conn: &Connection, title: &str) -> Result<Option<Task>> {
//     conn.query_row(
//         "SELECT id, title, done FROM tasks WHERE title = ?1",
//         params![title],
//         |row| {
//             let done_i: i64 = row.get(2)?;
//             Ok(Task {
//                 id: row.get(0)?,
//                 title: row.get(1)?,
//                 done: done_i != 0,
//             })
//         },
//     )
//     .optional()
//     .map_err(Into::into)
// }

#[cfg(test)]
mod tests {
    use super::Repository;
    use rusqlite::Connection;

    #[test]
    fn playground() {
        let conn = Connection::open_in_memory().unwrap();
        let mut repo = Repository::new(conn).unwrap();
        let r = repo.add_project("Project1").unwrap();

        let project = repo.get_project_by_id(r).unwrap();
        assert_eq!(project.name, "Project1");

        let active_view = repo.get_active_view_for_project(&project).unwrap();
        assert_eq!(active_view.name, "view");

        let window_manager_id = repo.get_window_manager_id(&active_view).unwrap();
        assert_eq!(window_manager_id, "1#Project1#view");

        // let mut project = Project::new("Project 1")
        //     .add_new_view("1", "View 1")
        //     .add_new_view("2", "View 2");
        // model.add_project(project);
        //
        // project = Project::new("Project 2");
        // project.add_view(View::new("3", "View 3"));
        // project.add_view(View::new("4", "View 4"));
        // model.add_project(project);
        //
        // assert_eq!(model.active, None);
        //
        // let v = model.find_view("Project 1", "View 1");
        // assert_eq!(v.unwrap().id, "1");
        // assert_eq!(v.unwrap().name, "View 1");
        //
        // model.set_active_project(model.find_project("Project 1").unwrap().clone());
        // assert_eq!(model.active.as_ref().unwrap().name, "Project 1");
        //
        // let v = model.find_view("Not Found", "Tag 1");
        // assert_eq!(v, None);
    }
}
