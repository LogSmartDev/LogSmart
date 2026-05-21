use anyhow::Result;
use sqlx::PgPool;

use super::types::*;

/// Retrieves various health metrics for the database.
///
/// # Errors
/// Returns an error if database queries for metrics fail.
pub async fn get_database_health(pool: &PgPool) -> Result<DatabaseHealthMetrics> {
    #[derive(sqlx::FromRow)]
    struct ConnectionStats {
        total: i64,
        active: i64,
        idle: i64,
        max_conn: i32,
    }

    #[derive(sqlx::FromRow)]
    struct DbSize {
        size_mb: f64,
    }

    #[derive(sqlx::FromRow)]
    struct TableCount {
        count: i64,
    }

    #[derive(sqlx::FromRow)]
    struct IndexCount {
        count: i64,
    }

    let conn_stats = sqlx::query_as::<_, ConnectionStats>(
        r"
        SELECT 
            COUNT(*) as total,
            COUNT(*) FILTER (WHERE state = 'active') as active,
            COUNT(*) FILTER (WHERE state = 'idle') as idle,
            (SELECT setting::int FROM pg_settings WHERE name = 'max_connections') as max_conn
        FROM pg_stat_activity
        WHERE datname = current_database()
        ",
    )
    .fetch_one(pool)
    .await?;

    let db_size = sqlx::query_as::<_, DbSize>(
        r"
        SELECT (pg_database_size(current_database())::float8 / (1024.0 * 1024.0)) as size_mb
        ",
    )
    .fetch_one(pool)
    .await?;

    let table_count = sqlx::query_as::<_, TableCount>(
        r"
        SELECT COUNT(*) as count
        FROM information_schema.tables
        WHERE table_schema = 'public' AND table_type = 'BASE TABLE'
        ",
    )
    .fetch_one(pool)
    .await?;

    let index_count = sqlx::query_as::<_, IndexCount>(
        r"
        SELECT COUNT(*) as count
        FROM pg_indexes
        WHERE schemaname = 'public'
        ",
    )
    .fetch_one(pool)
    .await?;

    Ok(DatabaseHealthMetrics {
        total_connections: conn_stats.total,
        active_connections: conn_stats.active,
        idle_connections: conn_stats.idle,
        max_connections: conn_stats.max_conn,
        database_size_mb: db_size.size_mb,
        table_count: table_count.count,
        index_count: index_count.count,
    })
}

/// Retrieves slow query information from `pg_stat_statements`.
///
/// # Errors
/// Returns an error if the database query fails.
pub async fn get_slow_queries(pool: &PgPool, limit: i64) -> Result<Vec<SlowQueryInfo>> {
    let queries = sqlx::query_as::<_, SlowQueryInfo>(
        r"
        SELECT 
            query,
            calls,
            total_exec_time as total_time_ms,
            mean_exec_time as mean_time_ms,
            max_exec_time as max_time_ms
        FROM pg_stat_statements
        WHERE query NOT LIKE '%pg_stat_statements%'
        ORDER BY mean_exec_time DESC
        LIMIT $1
        ",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    Ok(queries)
}

/// Retrieves index usage statistics for the database.
///
/// # Errors
/// Returns an error if the database query fails.
pub async fn get_index_usage(pool: &PgPool) -> Result<Vec<IndexUsageStats>> {
    let stats = sqlx::query_as::<_, IndexUsageStats>(
        r"
        SELECT 
            t.relname as table_name,
            i.relname as index_name,
            x.idx_scan as index_scans,
            x.idx_tup_read as rows_read,
            x.idx_tup_fetch as rows_fetched
        FROM pg_stat_user_indexes x
        JOIN pg_class i ON i.oid = x.indexrelid
        JOIN pg_class t ON t.oid = x.relid
        ORDER BY x.idx_scan DESC
        ",
    )
    .fetch_all(pool)
    .await?;

    Ok(stats)
}

/// Retrieves table and index sizes for the database.
///
/// # Errors
/// Returns an error if the database query fails.
pub async fn get_table_sizes(pool: &PgPool) -> Result<Vec<TableSizeInfo>> {
    #[derive(sqlx::FromRow)]
    struct TableSizeRow {
        table_name: String,
        row_count: i64,
        total_size_mb: f64,
        table_size_mb: f64,
        index_size_mb: f64,
    }

    let sizes = sqlx::query_as::<_, TableSizeRow>(
        r"
        SELECT 
            relname as table_name,
            n_live_tup as row_count,
            (pg_total_relation_size(schemaname||'.'||relname)::float8 / (1024*1024)) as total_size_mb,
            (pg_relation_size(schemaname||'.'||relname)::float8 / (1024*1024)) as table_size_mb,
            (pg_indexes_size(schemaname||'.'||relname)::float8 / (1024*1024)) as index_size_mb
        FROM pg_stat_user_tables
        WHERE schemaname = 'public'
        ORDER BY pg_total_relation_size(schemaname||'.'||relname) DESC
        ",
    )
    .fetch_all(pool)
    .await?;

    Ok(sizes
        .into_iter()
        .map(|s| TableSizeInfo {
            table_name: s.table_name,
            row_count: s.row_count,
            total_size_mb: s.total_size_mb,
            table_size_mb: s.table_size_mb,
            index_size_mb: s.index_size_mb,
        })
        .collect())
}

/// Identifies indexes that have never been used.
///
/// # Errors
/// Returns an error if the database query fails.
pub async fn check_unused_indexes(pool: &PgPool) -> Result<Vec<String>> {
    #[derive(sqlx::FromRow)]
    struct UnusedIndex {
        index_name: String,
    }

    let unused = sqlx::query_as::<_, UnusedIndex>(
        r"
        SELECT i.relname as index_name
        FROM pg_stat_user_indexes ui
        JOIN pg_class i ON i.oid = ui.indexrelid
        JOIN pg_class t ON t.oid = ui.relid
        WHERE ui.idx_scan = 0
        AND i.relname NOT LIKE '%_pkey%'
        ORDER BY i.relname
        ",
    )
    .fetch_all(pool)
    .await?;

    Ok(unused.into_iter().map(|u| u.index_name).collect())
}
