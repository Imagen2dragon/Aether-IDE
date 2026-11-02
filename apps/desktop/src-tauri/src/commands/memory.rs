use database_engine::{DatabaseEngine, ProjectMemoryNote};
use parking_lot::Mutex;
use std::sync::Arc;
use tauri::{AppHandle, Manager};

pub type SharedDatabase = Arc<Mutex<DatabaseEngine>>;

#[tauri::command]
pub async fn memory_save_note(
    app: AppHandle,
    title: String,
    content: String,
    tags: Vec<String>,
) -> Result<ProjectMemoryNote, String> {
    let db_arc = app.state::<SharedDatabase>();
    let db = db_arc.lock();

    let now = chrono::Utc::now().to_rfc3339();
    let note = ProjectMemoryNote {
        id: format!("note-{}", uuid::Uuid::new_v4().simple()),
        title,
        content,
        tags,
        created_at: now.clone(),
        updated_at: now,
    };

    db.save_memory_note(&note)
        .map_err(|e| format!("Failed to save memory note: {}", e))?;

    Ok(note)
}

#[tauri::command]
pub async fn memory_get_notes(app: AppHandle) -> Result<Vec<ProjectMemoryNote>, String> {
    let db_arc = app.state::<SharedDatabase>();
    let db = db_arc.lock();

    db.get_memory_notes()
        .map_err(|e| format!("Failed to retrieve memory notes: {}", e))
}

#[tauri::command]
pub async fn memory_delete_note(app: AppHandle, id: String) -> Result<bool, String> {
    let db_arc = app.state::<SharedDatabase>();
    let db = db_arc.lock();

    db.delete_memory_note(&id)
        .map_err(|e| format!("Failed to delete memory note: {}", e))
}
