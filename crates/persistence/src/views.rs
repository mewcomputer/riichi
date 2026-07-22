use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use uuid::Uuid;

use crate::Error;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct SavedViewRecord {
    pub id: Uuid,
    pub account_id: Uuid,
    pub project_id: Option<Uuid>,
    pub visibility: String,
    pub pinned: bool,
    pub name: String,
    pub filters: Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl super::Database {
    pub async fn list_saved_views(&self, account_id: Uuid) -> Result<Vec<SavedViewRecord>, Error> {
        Ok(sqlx::query_as::<_, SavedViewRecord>(
            "SELECT v.id, v.account_id, v.project_id, v.visibility,
                    EXISTS (SELECT 1 FROM human_saved_view_pins p
                            WHERE p.view_id = v.id AND p.account_id = $1) AS pinned,
                    v.name, v.filters, v.created_at, v.updated_at
             FROM human_saved_views v
             WHERE v.account_id = $1 AND v.visibility = 'personal'
             ORDER BY lower(name), id",
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn save_view(
        &self,
        account_id: Uuid,
        name: &str,
        filters: Value,
    ) -> Result<SavedViewRecord, Error> {
        Ok(sqlx::query_as::<_, SavedViewRecord>(
            "INSERT INTO human_saved_views (id, account_id, name, filters, visibility)
             VALUES ($1, $2, $3, $4, 'personal')
             ON CONFLICT (account_id, name) DO UPDATE
             SET filters = EXCLUDED.filters, updated_at = now()
             RETURNING id, account_id, project_id, visibility, false AS pinned, name, filters, created_at, updated_at",
        )
        .bind(Uuid::now_v7())
        .bind(account_id)
        .bind(name)
        .bind(filters)
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn delete_saved_view(&self, account_id: Uuid, view_id: Uuid) -> Result<bool, Error> {
        Ok(
            sqlx::query("DELETE FROM human_saved_views WHERE id = $1 AND account_id = $2")
                .bind(view_id)
                .bind(account_id)
                .execute(&self.pool)
                .await?
                .rows_affected()
                > 0,
        )
    }

    pub async fn list_project_saved_views(
        &self,
        project_id: Uuid,
        account_id: Uuid,
    ) -> Result<Vec<SavedViewRecord>, Error> {
        Ok(sqlx::query_as::<_, SavedViewRecord>(
            "SELECT v.id, v.account_id, v.project_id, v.visibility,
                    EXISTS (SELECT 1 FROM human_saved_view_pins p
                            WHERE p.view_id = v.id AND p.account_id = $2) AS pinned,
                    v.name, v.filters, v.created_at, v.updated_at
             FROM human_saved_views v
             WHERE v.project_id = $1 AND v.visibility = 'project'
             ORDER BY lower(name), id",
        )
        .bind(project_id)
        .bind(account_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn save_project_view(
        &self,
        project_id: Uuid,
        account_id: Uuid,
        name: &str,
        filters: Value,
    ) -> Result<SavedViewRecord, Error> {
        Ok(sqlx::query_as::<_, SavedViewRecord>(
            "INSERT INTO human_saved_views (id, account_id, project_id, visibility, name, filters)
             VALUES ($1, $2, $3, 'project', $4, $5)
             ON CONFLICT (project_id, lower(name)) WHERE visibility = 'project'
             DO UPDATE SET filters = EXCLUDED.filters, updated_at = now()
             RETURNING id, account_id, project_id, visibility, false AS pinned, name, filters, created_at, updated_at",
        )
        .bind(Uuid::now_v7())
        .bind(account_id)
        .bind(project_id)
        .bind(name)
        .bind(filters)
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn delete_project_saved_view(
        &self,
        project_id: Uuid,
        view_id: Uuid,
        account_id: Uuid,
        can_manage: bool,
    ) -> Result<bool, Error> {
        let result = if can_manage {
            sqlx::query(
                "DELETE FROM human_saved_views
                 WHERE id = $1 AND project_id = $2 AND visibility = 'project'",
            )
            .bind(view_id)
            .bind(project_id)
            .execute(&self.pool)
            .await?
        } else {
            sqlx::query(
                "DELETE FROM human_saved_views
                 WHERE id = $1 AND project_id = $2 AND account_id = $3 AND visibility = 'project'",
            )
            .bind(view_id)
            .bind(project_id)
            .bind(account_id)
            .execute(&self.pool)
            .await?
        };
        Ok(result.rows_affected() > 0)
    }

    pub async fn set_personal_saved_view_pinned(
        &self,
        account_id: Uuid,
        view_id: Uuid,
        pinned: bool,
    ) -> Result<bool, Error> {
        if pinned {
            let inserted = sqlx::query(
                "INSERT INTO human_saved_view_pins (account_id, view_id)
                 SELECT $1, id FROM human_saved_views
                 WHERE id = $2 AND account_id = $1 AND visibility = 'personal'
                 ON CONFLICT DO NOTHING",
            )
            .bind(account_id)
            .bind(view_id)
            .execute(&self.pool)
            .await?;
            Ok(inserted.rows_affected() > 0
                || sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS (SELECT 1 FROM human_saved_view_pins WHERE account_id = $1 AND view_id = $2)",
                )
                .bind(account_id)
                .bind(view_id)
                .fetch_one(&self.pool)
                .await?)
        } else {
            Ok(sqlx::query(
                "DELETE FROM human_saved_view_pins
                 WHERE account_id = $1 AND view_id = $2
                   AND EXISTS (SELECT 1 FROM human_saved_views WHERE id = $2 AND account_id = $1 AND visibility = 'personal')",
            )
            .bind(account_id)
            .bind(view_id)
            .execute(&self.pool)
            .await?
            .rows_affected()
                > 0)
        }
    }

    pub async fn set_project_saved_view_pinned(
        &self,
        project_id: Uuid,
        account_id: Uuid,
        view_id: Uuid,
        pinned: bool,
    ) -> Result<bool, Error> {
        if pinned {
            let inserted = sqlx::query(
                "INSERT INTO human_saved_view_pins (account_id, view_id)
                 SELECT $1, id FROM human_saved_views
                 WHERE id = $2 AND project_id = $3 AND visibility = 'project'
                 ON CONFLICT DO NOTHING",
            )
            .bind(account_id)
            .bind(view_id)
            .bind(project_id)
            .execute(&self.pool)
            .await?;
            Ok(inserted.rows_affected() > 0
                || sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS (SELECT 1 FROM human_saved_view_pins WHERE account_id = $1 AND view_id = $2)",
                )
                .bind(account_id)
                .bind(view_id)
                .fetch_one(&self.pool)
                .await?)
        } else {
            Ok(sqlx::query(
                "DELETE FROM human_saved_view_pins
                 WHERE account_id = $1 AND view_id = $2
                   AND EXISTS (SELECT 1 FROM human_saved_views WHERE id = $2 AND project_id = $3 AND visibility = 'project')",
            )
            .bind(account_id)
            .bind(view_id)
            .bind(project_id)
            .execute(&self.pool)
            .await?
            .rows_affected()
                > 0)
        }
    }
}
