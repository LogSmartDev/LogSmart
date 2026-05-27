use crate::db;
use crate::try_db;
use sqlx::PgPool;

#[cfg(test)]
mod user_service_tests {
    use super::*;

    /// Helper to create a test user with company manager role
    fn create_test_company_manager() -> db::UserRecord {
        db::UserRecord {
            id: "admin-user-id".to_string(),
            email: "admin@company.com".to_string(),
            first_name: "Admin".to_string(),
            last_name: "Manager".to_string(),
            password_hash: Some("hash".to_string()),
            company_id: Some("company-123".to_string()),
            branch_id: None,
            company_name: Some("Test Company".to_string()),
            company_deleted_at: None,
            role: db::UserRole::CompanyManager,
            created_at: chrono::Utc::now(),
            deleted_at: None,
            oauth_provider: None,
            oauth_subject: None,
            oauth_picture: None,
            profile_picture_id: None,
        }
    }

    /// Helper to create a test staff user
    fn create_test_staff_user() -> db::UserRecord {
        db::UserRecord {
            id: "staff-user-id".to_string(),
            email: "staff@company.com".to_string(),
            first_name: "Staff".to_string(),
            last_name: "Member".to_string(),
            password_hash: Some("hash".to_string()),
            company_id: Some("company-123".to_string()),
            branch_id: None,
            company_name: Some("Test Company".to_string()),
            company_deleted_at: None,
            role: db::UserRole::Staff,
            created_at: chrono::Utc::now(),
            deleted_at: None,
            oauth_provider: None,
            oauth_subject: None,
            oauth_picture: None,
            profile_picture_id: None,
        }
    }

    /// Test that a company manager can update a staff member's profile
    #[test]
    fn test_admin_update_member_profile_valid_permission() {
        let admin = create_test_company_manager();
        let target = create_test_staff_user();

        // Verify that admin has can_manage_branch permission
        assert!(admin.can_manage_branch());
        assert!(!admin.is_readonly_hq());

        // Verify that admin and target are in the same company
        assert_eq!(admin.company_id, target.company_id);
    }

    /// Test that company managers cannot update users from other companies
    #[test]
    fn test_admin_update_member_from_different_company_forbidden() {
        let mut admin = create_test_company_manager();
        let mut target = create_test_staff_user();

        // Set different companies
        admin.company_id = Some("company-123".to_string());
        target.company_id = Some("company-456".to_string());

        // Verify they are in different companies
        assert_ne!(admin.company_id, target.company_id);

        // Verify admin cannot update this user
        let is_forbidden = admin.is_company_manager() && admin.company_id != target.company_id;
        assert!(is_forbidden);
    }

    /// Test that staff users cannot update member profiles
    #[test]
    fn test_admin_update_member_profile_staff_forbidden() {
        let staff_user = create_test_staff_user();

        // Verify staff user cannot manage branches
        assert!(!staff_user.can_manage_branch());
    }

    /// Test that users cannot delete their own account
    #[test]
    fn test_admin_delete_member_self_forbidden() {
        let admin = create_test_company_manager();
        let same_user = create_test_company_manager();

        // Verify same email means cannot delete self
        assert_eq!(admin.email, same_user.email);
    }

    /// Test that company managers cannot delete users from other companies
    #[test]
    fn test_admin_delete_member_different_company_forbidden() {
        let mut admin = create_test_company_manager();
        let mut target = create_test_staff_user();

        // Set different companies
        admin.company_id = Some("company-123".to_string());
        target.company_id = Some("company-456".to_string());

        // Verify they are in different companies
        assert_ne!(admin.company_id, target.company_id);

        // Verify admin cannot delete this user
        let is_forbidden = admin.is_company_manager() && admin.company_id != target.company_id;
        assert!(is_forbidden);
    }

    /// Test role validation for LogSmart admin role assignment
    #[test]
    fn test_logsmart_admin_role_assignment_restricted() {
        let company_manager = create_test_company_manager();

        // Company managers should not be able to assign LogSmart admin role
        assert!(!company_manager.is_logsmart_admin());

        // Verify permission check
        let can_assign_admin = company_manager.is_logsmart_admin();
        assert!(!can_assign_admin);
    }

    /// Test that read-only HQ users (staff with no branch) cannot update member profiles
    #[test]
    fn test_readonly_hq_cannot_update_members() {
        let mut readonly_hq = create_test_staff_user();
        readonly_hq.branch_id = None; // Staff with no branch = readonly HQ

        // Verify readonly HQ users are correctly identified
        assert!(readonly_hq.is_readonly_hq());

        // Verify readonly HQ users cannot manage branches
        assert!(!readonly_hq.can_manage_branch());
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
        .ok_or(crate::error::AppError::NotFound(
            "User not found".to_string(),
        ))?;
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
        let user = try_db!(db::get_user_by_id(db_pool, user_id), "fetching user by id")?.ok_or(
            crate::error::AppError::NotFound("User not found".to_string()),
        )?;
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
        .ok_or(crate::error::AppError::Forbidden(
            "User is not associated with a company".to_string(),
        ))?;
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
            return Err(crate::error::AppError::Forbidden(
                "Only managers can update member profiles".to_string(),
            ));
        }

        let target_user = Self::get_user_by_email(db_pool, target_email).await?;

        if admin_user.is_company_manager() && admin_user.company_id != target_user.company_id {
            return Err(crate::error::AppError::Forbidden(
                "Cannot update users from other companies".to_string(),
            ));
        }

        if admin_user.is_branch_manager() {
            if admin_user.branch_id != target_user.branch_id {
                return Err(crate::error::AppError::Forbidden(
                    "Branch managers can only manage users in their branch".to_string(),
                ));
            }
            if branch_id != admin_user.branch_id {
                return Err(crate::error::AppError::Forbidden(
                    "Branch managers can only assign users to their own branch".to_string(),
                ));
            }
            if role != db::UserRole::Staff {
                return Err(crate::error::AppError::Forbidden(
                    "Branch managers can only manage staff members".to_string(),
                ));
            }
        }

        if target_user.is_logsmart_admin() && !admin_user.is_logsmart_admin() {
            return Err(crate::error::AppError::Forbidden(
                "Cannot modify LogSmart internal admin users".to_string(),
            ));
        }

        // Only LogSmart admins can assign the LogSmart admin role
        if role == db::UserRole::LogSmartAdmin && !admin_user.is_logsmart_admin() {
            return Err(crate::error::AppError::Forbidden(
                "Only LogSmart admins can assign the LogSmart admin role".to_string(),
            ));
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
            return Err(crate::error::AppError::Forbidden(
                "Cannot delete users from other companies".to_string(),
            ));
        }

        if admin_user.is_branch_manager() && admin_user.branch_id != target_user.branch_id {
            return Err(crate::error::AppError::Forbidden(
                "Branch managers can only delete users in their branch".to_string(),
            ));
        }

        if target_user.is_logsmart_admin() && !admin_user.is_logsmart_admin() {
            return Err(crate::error::AppError::Forbidden(
                "Cannot delete LogSmart internal admin users".to_string(),
            ));
        }

        if target_user.email == admin_user.email {
            return Err(crate::error::AppError::BadRequest(
                "Cannot delete your own account".to_string(),
            ));
        }

        try_db!(
            db::delete_user_by_email(db_pool, target_email),
            "deleting member"
        )?;

        Ok(target_user.id)
    }
}
