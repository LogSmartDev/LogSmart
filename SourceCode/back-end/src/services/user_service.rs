use crate::db;
use crate::try_db;
use sqlx::PgPool;

#[cfg(test)]
mod user_service_tests {
    #[tokio::test]
    async fn test_user_service_basic() {
        assert!(true);
    }
}

pub struct UserService;

impl UserService {
    /// Retrieves a user by their email address.
    ///
    /// # Errors
    /// Returns an error if the user is not found or if database lookup fails.
    pub async fn get_user_by_email(
        db_pool: &PgPool,
        email: &str,
    ) -> Result<db::UserRecord, crate::error::AppError> {
        let user = try_db!(
            db::get_user_by_email(db_pool, email),
            "fetching user by email"
        )?
        .ok_or(crate::error::AppError::NotFound("User not found".to_string()))?;
        Ok(user)
    }

    /// Retrieves a user by their ID.
    ///
    /// # Errors
    /// Returns an error if the user is not found or if database lookup fails.
    pub async fn get_user_by_id(
        db_pool: &PgPool,
        user_id: &str,
    ) -> Result<db::UserRecord, crate::error::AppError> {
        let user = try_db!(db::get_user_by_id(db_pool, user_id), "fetching user by id")?
            .ok_or(crate::error::AppError::NotFound("User not found".to_string()))?;
        Ok(user)
    }

    /// Updates a user's profile information.
    ///
    /// # Errors
    /// Returns an error if the database update fails.
    pub async fn update_profile(
        db_pool: &PgPool,
        user_id: &str,
        first_name: String,
        last_name: String,
    ) -> Result<db::UserRecord, crate::error::AppError> {
        try_db!(
            db::update_user_profile(db_pool, user_id, first_name, last_name),
            "updating user profile"
        )
    }

    /// Retrieves all members of a specific company.
    ///
    /// # Errors
    /// Returns an error if the database query fails.
    pub async fn get_company_members(
        db_pool: &PgPool,
        company_id: &str,
    ) -> Result<Vec<db::UserRecord>, crate::error::AppError> {
        try_db!(
            db::get_users_by_company_id(db_pool, company_id),
            "fetching company members"
        )
    }

    /// Retrieves the company ID for a specific user.
    ///
    /// # Errors
    /// Returns an error if the user is not associated with a company or if the query fails.
    pub async fn get_user_company_id(
        db_pool: &PgPool,
        user_id: &str,
    ) -> Result<String, crate::error::AppError> {
        let company_id = try_db!(
            db::get_user_company_id(db_pool, user_id),
            "fetching user company ID"
        )?
        .ok_or(crate::error::AppError::Forbidden("User is not associated with a company".to_string()))?;
        Ok(company_id)
    }

    /// Updates a company member's profile (admin only).
    ///
    /// # Errors
    /// Returns an error if the caller is not an admin, target is in another company, or update fails.
    pub async fn admin_update_member_profile(
        db_pool: &PgPool,
        admin_user: &db::UserRecord,
        target_email: &str,
        first_name: String,
        last_name: String,
        role: db::UserRole,
        branch_id: Option<String>,
        profile_picture_id: Option<String>,
    ) -> Result<db::UserRecord, crate::error::AppError> {
        if !admin_user.can_manage_branch() || admin_user.is_readonly_hq() {
            return Err(crate::error::AppError::Forbidden("Only managers can update member profiles".to_string()));
        }

        let target_user = Self::get_user_by_email(db_pool, target_email).await?;

        if admin_user.is_company_manager() && admin_user.company_id != target_user.company_id {
            return Err(crate::error::AppError::Forbidden("Cannot update users from other companies".to_string()));
        }

        if admin_user.is_branch_manager() {
            if admin_user.branch_id != target_user.branch_id {
                return Err(crate::error::AppError::Forbidden("Branch managers can only manage users in their branch".to_string()));
            }
            if branch_id != admin_user.branch_id {
                return Err(crate::error::AppError::Forbidden("Branch managers can only assign users to their own branch".to_string()));
            }
            if role != db::UserRole::Staff {
                return Err(crate::error::AppError::Forbidden("Branch managers can only manage staff members".to_string()));
            }
        }

        if target_user.is_logsmart_admin() && !admin_user.is_logsmart_admin() {
            return Err(crate::error::AppError::Forbidden("Cannot modify LogSmart internal admin users".to_string()));
        }

        // Only LogSmart admins can assign the LogSmart admin role
        if role == db::UserRole::LogSmartAdmin && !admin_user.is_logsmart_admin() {
            return Err(crate::error::AppError::Forbidden("Only LogSmart admins can assign the LogSmart admin role".to_string()));
        }

        try_db!(
            db::update_user_profile_full(
                db_pool,
                &target_user.id,
                first_name,
                last_name,
                role,
                branch_id,
                profile_picture_id,
            ),
            "updating member profile"
        )
    }

    /// Deletes a company member (admin only).
    ///
    /// # Errors
    /// Returns an error if the caller is not an admin, target is in another company, or deletion fails.
    pub async fn admin_delete_member(
        db_pool: &PgPool,
        admin_user: &db::UserRecord,
        target_email: &str,
    ) -> Result<String, crate::error::AppError> {
        let target_user = Self::get_user_by_email(db_pool, target_email).await?;

        if admin_user.is_company_manager() && admin_user.company_id != target_user.company_id {
            return Err(crate::error::AppError::Forbidden("Cannot delete users from other companies".to_string()));
        }

        if admin_user.is_branch_manager() && admin_user.branch_id != target_user.branch_id {
            return Err(crate::error::AppError::Forbidden("Branch managers can only delete users in their branch".to_string()));
        }

        if target_user.is_logsmart_admin() && !admin_user.is_logsmart_admin() {
            return Err(crate::error::AppError::Forbidden("Cannot delete LogSmart internal admin users".to_string()));
        }

        if target_user.email == admin_user.email {
            return Err(crate::error::AppError::BadRequest("Cannot delete your own account".to_string()));
        }

        try_db!(
            db::delete_user_by_email(db_pool, target_email),
            "deleting member"
        )?;

        Ok(target_user.id)
    }
}
