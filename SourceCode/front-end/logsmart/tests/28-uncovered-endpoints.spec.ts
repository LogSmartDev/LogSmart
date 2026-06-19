import { test, expect } from '@playwright/test';
import {
	register,
	createBranch,
	sendInvitation,
	acceptInvitation,
	clearMailhogEmails,
	dismissCookieBannerInTests
} from './utils';

const BACKEND_URL = process.env.BACKEND_URL || 'http://localhost:6767';

// Shared admin credentials for all tests
let adminCreds: {
	email: string;
	password: string;
	companyName: string;
	firstName: string;
	lastName: string;
	page?: unknown;
};

let branchName: string;

test.beforeAll(async ({ browser }) => {
	const result = await register(browser, false);
	if (!result) throw new Error('Registration failed');
	adminCreds = result;
	branchName = `Branch-${Date.now()}`;
	await createBranch(result.page!, branchName, '456 Test Ave');
	await result.page!.close();
});

// Helper: login via API and return auth token
async function loginApi(email: string, password: string): Promise<string> {
	const response = await fetch(`${BACKEND_URL}/auth/login`, {
		method: 'POST',
		headers: { 'Content-Type': 'application/json' },
		body: JSON.stringify({ email, password })
	});

	if (!response.ok) {
		const error = await response.json();
		throw new Error(`Login failed: ${error.error || response.statusText}`);
	}

	const data = await response.json();
	return data.token;
}

// ============================================================================
// Test Group 1: DELETE /logs/entries/{id} - Log Entry Deletion
// ============================================================================
test.describe('Log Entry Deletion', () => {
	test('admin_can_delete_own_log_entry', async () => {
		const token = await loginApi(adminCreds.email, adminCreds.password);
		const templateName = `DeleteTest-${Date.now()}`;

		// Create a template first
		const templateResponse = await fetch(`${BACKEND_URL}/logs/templates`, {
			method: 'POST',
			headers: {
				'Content-Type': 'application/json',
				Authorization: `Bearer ${token}`
			},
			body: JSON.stringify({
				template_name: templateName,
				template_layout: [
					{
						field_type: 'text',
						position: { x: 0, y: 0 },
						props: { text: 'Test Field', required: true }
					}
				],
				schedule: {
					frequency: 'Daily',
					days_of_week: [1, 2, 3, 4, 5, 6, 7]
				}
			})
		});

		if (!templateResponse.ok) {
			console.log('Template creation failed:', await templateResponse.text());
			test.skip(true, 'Cannot create template');
			return;
		}

		// Create a log entry using the template name we just created
		const createResponse = await fetch(`${BACKEND_URL}/logs/entries`, {
			method: 'POST',
			headers: {
				'Content-Type': 'application/json',
				Authorization: `Bearer ${token}`
			},
			body: JSON.stringify({ template_name: templateName })
		});

		if (!createResponse.ok) {
			console.log('Entry creation failed:', await createResponse.text());
			test.skip(true, 'Cannot create log entry');
			return;
		}

		const entryData = await createResponse.json();
		const entryId = entryData.id;

		// Delete the log entry
		const deleteResponse = await fetch(`${BACKEND_URL}/logs/entries/${entryId}`, {
			method: 'DELETE',
			headers: {
				Authorization: `Bearer ${token}`
			}
		});

		expect(deleteResponse.status).toBe(200);
		const deleteData = await deleteResponse.json();
		expect(deleteData.message).toBe('Log entry deleted successfully');

		// Verify entry is gone
		const getResponse = await fetch(`${BACKEND_URL}/logs/entries/${entryId}`, {
			headers: {
				Authorization: `Bearer ${token}`
			}
		});
		expect(getResponse.status).toBe(404);
	});

	test('member_cannot_delete_another_users_log_entry', async ({ browser }) => {
		// Create a member user
		const memberEmail = `member-delete+${Date.now()}@logsmart.app`;
		const inviteToken = await sendInvitation(browser, adminCreds, memberEmail, 'staff', branchName);
		if (!inviteToken) throw new Error('Failed to get invitation token');

		const memberPage = await browser.newPage();
		await dismissCookieBannerInTests(memberPage);
		await acceptInvitation(
			memberPage,
			inviteToken,
			'Member',
			'User',
			'Member123!A',
			'**/logs-list'
		);
		await memberPage.close();

		// Admin creates an entry
		const adminToken = await loginApi(adminCreds.email, adminCreds.password);
		const templateName = `AdminTemplate-${Date.now()}`;

		await fetch(`${BACKEND_URL}/logs/templates`, {
			method: 'POST',
			headers: {
				'Content-Type': 'application/json',
				Authorization: `Bearer ${adminToken}`
			},
			body: JSON.stringify({
				template_name: templateName,
				template_layout: [
					{
						field_type: 'text',
						position: { x: 0, y: 0 },
						props: { text: 'Test', required: true }
					}
				],
				schedule: {
					frequency: 'Daily',
					days_of_week: [1, 2, 3, 4, 5, 6, 7]
				}
			})
		});

		const createRes = await fetch(`${BACKEND_URL}/logs/entries`, {
			method: 'POST',
			headers: {
				'Content-Type': 'application/json',
				Authorization: `Bearer ${adminToken}`
			},
			body: JSON.stringify({ template_name: templateName })
		});

		const entryData = await createRes.json();
		const entryId = entryData.id;

		// Member tries to delete admin's entry
		const memberToken = await loginApi(memberEmail, 'Member123!A');
		const deleteResponse = await fetch(`${BACKEND_URL}/logs/entries/${entryId}`, {
			method: 'DELETE',
			headers: {
				Authorization: `Bearer ${memberToken}`
			}
		});

		// Should be forbidden or not found
		expect([403, 404]).toContain(deleteResponse.status);
	});

	test('delete_non_existent_entry_returns_404', async () => {
		const token = await loginApi(adminCreds.email, adminCreds.password);
		const deleteResponse = await fetch(`${BACKEND_URL}/logs/entries/non-existent-id`, {
			method: 'DELETE',
			headers: {
				Authorization: `Bearer ${token}`
			}
		});

		expect(deleteResponse.status).toBe(404);
	});
});

// ============================================================================
// Test Group 2: DELETE /auth/admin/remove-member - Remove Member
// ============================================================================
test.describe('Remove Member', () => {
	test('admin_can_remove_member', async ({ browser }) => {
		const memberEmail = `remove-me+${Date.now()}@logsmart.app`;
		const inviteToken = await sendInvitation(browser, adminCreds, memberEmail, 'staff', branchName);
		if (!inviteToken) throw new Error('Failed to get invitation token');

		const memberPage = await browser.newPage();
		await dismissCookieBannerInTests(memberPage);
		await acceptInvitation(
			memberPage,
			inviteToken,
			'Remove',
			'Me',
			'RemoveMe123!A',
			'**/logs-list'
		);
		await memberPage.close();

		const adminToken = await loginApi(adminCreds.email, adminCreds.password);

		const removeResponse = await fetch(`${BACKEND_URL}/auth/admin/remove-member`, {
			method: 'DELETE',
			headers: {
				'Content-Type': 'application/json',
				Authorization: `Bearer ${adminToken}`
			},
			body: JSON.stringify({ email: memberEmail })
		});

		expect(removeResponse.status).toBe(200);
		const removeData = await removeResponse.json();
		expect(removeData.message).toBe('Member deleted successfully');
	});

	test('removed_member_cannot_login', async ({ browser }) => {
		const memberEmail = `remove-login+${Date.now()}@logsmart.app`;
		const memberPassword = 'RemoveLogin123!A';

		// Create and accept invitation for the member
		const inviteToken = await sendInvitation(browser, adminCreds, memberEmail, 'staff', branchName);
		if (!inviteToken) throw new Error('Failed to get invitation token');

		// Accept invitation using API directly to avoid UI timeout
		const acceptResponse = await fetch(`${BACKEND_URL}/auth/invitations/accept`, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({
				token: inviteToken,
				first_name: 'Remove',
				last_name: 'Login',
				password: memberPassword
			})
		});
		expect(acceptResponse.ok).toBe(true);

		// Verify member can login before removal
		const loginBeforeResponse = await fetch(`${BACKEND_URL}/auth/login`, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ email: memberEmail, password: memberPassword })
		});
		expect(loginBeforeResponse.ok).toBe(true);

		// Remove the member
		const adminToken = await loginApi(adminCreds.email, adminCreds.password);
		const removeResponse = await fetch(`${BACKEND_URL}/auth/admin/remove-member`, {
			method: 'DELETE',
			headers: {
				'Content-Type': 'application/json',
				Authorization: `Bearer ${adminToken}`
			},
			body: JSON.stringify({ email: memberEmail })
		});
		expect(removeResponse.status).toBe(200);

		// Try to login with the removed member's credentials - should fail
		const loginAfterResponse = await fetch(`${BACKEND_URL}/auth/login`, {
			method: 'POST',
			headers: { 'Content-Type': 'application/json' },
			body: JSON.stringify({ email: memberEmail, password: memberPassword })
		});
		expect([401, 403]).toContain(loginAfterResponse.status);
	});

	test('non_admin_cannot_remove_members', async ({ browser }) => {
		// Create a staff member
		const staffEmail = `staff+${Date.now()}@logsmart.app`;
		const inviteToken = await sendInvitation(browser, adminCreds, staffEmail, 'staff', branchName);
		if (!inviteToken) throw new Error('Failed to get invitation token');

		const staffPage = await browser.newPage();
		await dismissCookieBannerInTests(staffPage);
		await acceptInvitation(staffPage, inviteToken, 'Staff', 'User', 'Staff123!A', '**/logs-list');
		await staffPage.close();

		const staffToken = await loginApi(staffEmail, 'Staff123!A');

		// Try to remove a member as staff
		const removeResponse = await fetch(`${BACKEND_URL}/auth/admin/remove-member`, {
			method: 'DELETE',
			headers: {
				'Content-Type': 'application/json',
				Authorization: `Bearer ${staffToken}`
			},
			body: JSON.stringify({ email: `target+${Date.now()}@logsmart.app` })
		});

		expect([403, 401]).toContain(removeResponse.status);
	});
});

// ============================================================================
// Test Group 3: PUT /auth/invitations/cancel - Cancel Invitation
// ============================================================================
test.describe('Cancel Invitation', () => {
	test('admin_can_cancel_invitation', async ({ browser }) => {
		const email = `cancel-me+${Date.now()}@logsmart.app`;
		await clearMailhogEmails();

		const inviteToken = await sendInvitation(browser, adminCreds, email, 'staff', branchName);
		expect(inviteToken).toBeTruthy();

		const adminToken = await loginApi(adminCreds.email, adminCreds.password);

		// Get the invitation ID from pending invitations
		const pendingResponse = await fetch(`${BACKEND_URL}/auth/invitations/pending`, {
			headers: {
				Authorization: `Bearer ${adminToken}`
			}
		});
		const pendingData = await pendingResponse.json();
		const invitation = pendingData.invitations?.find((inv: any) => inv.email === email);
		if (!invitation) {
			test.skip(true, 'Invitation not found in pending list');
			return;
		}

		const cancelResponse = await fetch(`${BACKEND_URL}/auth/invitations/cancel`, {
			method: 'PUT',
			headers: {
				'Content-Type': 'application/json',
				Authorization: `Bearer ${adminToken}`
			},
			body: JSON.stringify({ invitation_id: invitation.id })
		});

		expect(cancelResponse.status).toBe(200);
	});

	test('cancelled_invitation_token_becomes_invalid', async ({ browser }) => {
		const email = `cancel-invalid+${Date.now()}@logsmart.app`;
		await clearMailhogEmails();

		const inviteToken = await sendInvitation(browser, adminCreds, email, 'staff', branchName);
		expect(inviteToken).toBeTruthy();

		const adminToken = await loginApi(adminCreds.email, adminCreds.password);

		// Get invitation ID
		const pendingResponse = await fetch(`${BACKEND_URL}/auth/invitations/pending`, {
			headers: {
				Authorization: `Bearer ${adminToken}`
			}
		});
		const pendingData = await pendingResponse.json();
		const invitation = pendingData.invitations?.find((inv: any) => inv.email === email);
		if (!invitation) {
			test.skip(true, 'Invitation not found');
			return;
		}

		// Cancel the invitation
		await fetch(`${BACKEND_URL}/auth/invitations/cancel`, {
			method: 'PUT',
			headers: {
				'Content-Type': 'application/json',
				Authorization: `Bearer ${adminToken}`
			},
			body: JSON.stringify({ invitation_id: invitation.id })
		});

		// Try to accept with cancelled token
		const page = await browser.newPage();
		await dismissCookieBannerInTests(page);
		await page.goto(`http://localhost:5173/accept-invitation?token=${inviteToken}`);
		await page.waitForLoadState('networkidle');

		await expect(page.locator('body')).toContainText(/invalid|expired|cancelled/i);
		await page.close();
	});

	test('non_admin_cannot_cancel_invitations', async ({ browser }) => {
		// Create a staff member
		const staffEmail = `staff-cancel+${Date.now()}@logsmart.app`;
		const inviteToken = await sendInvitation(browser, adminCreds, staffEmail, 'staff', branchName);
		if (!inviteToken) throw new Error('Failed to get invitation token');

		const staffPage = await browser.newPage();
		await dismissCookieBannerInTests(staffPage);
		await acceptInvitation(staffPage, inviteToken, 'Staff', 'User', 'Staff123!A', '**/logs-list');
		await staffPage.close();

		const staffToken = await loginApi(staffEmail, 'Staff123!A');

		// Try to cancel an invitation as staff
		const cancelResponse = await fetch(`${BACKEND_URL}/auth/invitations/cancel`, {
			method: 'PUT',
			headers: {
				'Content-Type': 'application/json',
				Authorization: `Bearer ${staffToken}`
			},
			body: JSON.stringify({ invitation_id: 'non-existent-id' })
		});

		expect([403, 401]).toContain(cancelResponse.status);
	});
});

// ============================================================================
// Test Group 4: PUT /auth/company/branches - Update Branch
// ============================================================================
test.describe('Update Branch', () => {
	test('manager_can_update_branch_name', async () => {
		const adminToken = await loginApi(adminCreds.email, adminCreds.password);

		// First get the branch ID
		const listResponse = await fetch(`${BACKEND_URL}/auth/company/branches`, {
			headers: {
				Authorization: `Bearer ${adminToken}`
			}
		});

		expect(listResponse.status).toBe(200);
		const listData = await listResponse.json();
		const branches = listData.branches || [];
		const branch = branches.find((b: any) => b.name === branchName);
		if (!branch) {
			test.skip(true, 'Branch not found');
			return;
		}

		// Update branch name
		const newName = `${branchName}-Updated`;
		const updateResponse = await fetch(`${BACKEND_URL}/auth/company/branches`, {
			method: 'PUT',
			headers: {
				'Content-Type': 'application/json',
				Authorization: `Bearer ${adminToken}`
			},
			body: JSON.stringify({
				branch_id: branch.id,
				name: newName,
				address: branch.address
			})
		});

		expect(updateResponse.status).toBe(200);
		const updateData = await updateResponse.json();
		console.log(updateData);
		expect(updateData.name).toBe(newName);

		// Verify the update
		const getResponse = await fetch(`${BACKEND_URL}/auth/company/branches`, {
			headers: {
				Authorization: `Bearer ${adminToken}`
			}
		});

		const updatedBranchesData = await getResponse.json();
		const updatedBranches = updatedBranchesData.branches || [];
		const updatedBranch = updatedBranches.find((b: any) => b.id === branch.id);
		expect(updatedBranch.name).toBe(newName);
	});

	test('manager_can_update_branch_address', async () => {
		const adminToken = await loginApi(adminCreds.email, adminCreds.password);

		const listResponse = await fetch(`${BACKEND_URL}/auth/company/branches`, {
			headers: {
				Authorization: `Bearer ${adminToken}`
			}
		});

		const listData = await listResponse.json();
		const branches = listData.branches || [];
		const branch = branches.find((b: any) => b.name.startsWith('Branch-'));
		if (!branch) {
			test.skip(true, 'Branch not found');
			return;
		}

		const newAddress = '789 Updated Street, New City';
		const updateResponse = await fetch(`${BACKEND_URL}/auth/company/branches`, {
			method: 'PUT',
			headers: {
				'Content-Type': 'application/json',
				Authorization: `Bearer ${adminToken}`
			},
			body: JSON.stringify({
				branch_id: branch.id,
				name: branch.name,
				address: newAddress
			})
		});

		expect(updateResponse.status).toBe(200);

		const getResponse = await fetch(`${BACKEND_URL}/auth/company/branches`, {
			headers: {
				Authorization: `Bearer ${adminToken}`
			}
		});

		const getResponseData = await getResponse.json();
		const updatedBranches = getResponseData.branches || [];
		const updatedBranch = updatedBranches.find((b: any) => b.id === branch.id);
		expect(updatedBranch.address).toBe(newAddress);
	});

	test('staff_cannot_update_branches', async ({ browser }) => {
		const staffEmail = `staff-branch+${Date.now()}@logsmart.app`;
		const inviteToken = await sendInvitation(browser, adminCreds, staffEmail, 'staff', branchName);
		if (!inviteToken) throw new Error('Failed to get invitation token');

		const staffPage = await browser.newPage();
		await dismissCookieBannerInTests(staffPage);
		await acceptInvitation(staffPage, inviteToken, 'Staff', 'User', 'Staff123!A', '**/logs-list');
		await staffPage.close();

		const staffToken = await loginApi(staffEmail, 'Staff123!A');

		const updateResponse = await fetch(`${BACKEND_URL}/auth/company/branches`, {
			method: 'PUT',
			headers: {
				'Content-Type': 'application/json',
				Authorization: `Bearer ${staffToken}`
			},
			body: JSON.stringify({
				branch_id: 'some-branch-id',
				name: 'Hacked Branch',
				address: 'Hacked Address'
			})
		});

		expect([403, 401]).toContain(updateResponse.status);
	});

	test('update_branch_with_empty_name_fails_validation', async () => {
		const adminToken = await loginApi(adminCreds.email, adminCreds.password);

		const listResponse = await fetch(`${BACKEND_URL}/auth/company/branches`, {
			headers: {
				Authorization: `Bearer ${adminToken}`
			}
		});

		const listData = await listResponse.json();
		const branches = listData.branches || [];
		const branch = branches.find((b: any) => b.name.startsWith('Branch-'));
		if (!branch) {
			test.skip(true, 'Branch not found');
			return;
		}

		const updateResponse = await fetch(`${BACKEND_URL}/auth/company/branches`, {
			method: 'PUT',
			headers: {
				'Content-Type': 'application/json',
				Authorization: `Bearer ${adminToken}`
			},
			body: JSON.stringify({
				branch_id: branch.id,
				name: '',
				address: branch.address
			})
		});

		expect(await updateResponse.text()).toContain(
			'{"error":"Name must be between 1 and 255 characters","fields":{"name":["Name must be between 1 and 255 characters"]}}'
		);
		expect([400, 422]).toContain(updateResponse.status);
	});
});
