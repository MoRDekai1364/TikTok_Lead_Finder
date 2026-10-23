use anyhow::Result;
use sqlx::Row;
use crate::db::Database;

impl Database {
    pub async fn set_claim_cluster(&self, claim_id: i64, cluster_id: i64) -> Result<()> {
        sqlx::query("UPDATE claims SET cluster_id = ?2 WHERE id = ?1")
            .bind(claim_id)
            .bind(cluster_id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    pub async fn latest_keep_discard_decisions(&self) -> Result<Vec<(String, bool)>> {
        let rows = sqlx::query(
            "SELECT d.video_id AS video_id, d.action AS action
             FROM decisions d
             WHERE d.id = (SELECT MAX(id) FROM decisions WHERE video_id = d.video_id)
               AND d.action IN ('keep', 'discard')",
        )
        .fetch_all(self.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| {
                let action: String = r.get("action");
                (r.get::<String, _>("video_id"), action == "keep")
            })
            .collect())
    }

    pub async fn refresh_creator_flagged_counts(&self, window_days: i64) -> Result<()> {
        let since = (chrono::Utc::now() - chrono::Duration::days(window_days)).to_rfc3339();
        sqlx::query(
            "UPDATE creators SET flagged_count = (
                SELECT COUNT(*) FROM videos v
                WHERE v.creator_id = creators.id
                  AND (v.lane = 'leads' OR v.state = 'KEPT')
                  AND v.discovered_at >= ?1
             )",
        )
        .bind(&since)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    pub async fn eligible_expansion_creators(
        &self,
        min_flagged: i64,
        max_depth: i64,
    ) -> Result<Vec<(String, i64, i64)>> {
        let rows = sqlx::query(
            "SELECT handle, expansion_depth, flagged_count FROM creators
             WHERE flagged_count >= ?1 AND expansion_depth < ?2
             ORDER BY flagged_count DESC, handle ASC",
        )
        .bind(min_flagged)
        .bind(max_depth)
        .fetch_all(self.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| {
                (
                    r.get::<String, _>("handle"),
                    r.get::<i64, _>("expansion_depth"),
                    r.get::<i64, _>("flagged_count"),
                )
            })
            .collect())
    }
}