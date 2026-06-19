import { test, expect } from '@playwright/test';
import { register, createBranch, getBranchDeletionToken, clearMailhogEmails } from './utils';

let adminCreds: {
	email: string;
	password: string;
	companyName: string;
	firstName: string;
	lastName: string;
};

const BRANCH_TO_DELETE = 'Branch To Delete';

test.beforeAll(async ({ browser }) => {
	await clearMailhogEmails();
	const creds = await register(browser);
	if (!creds) throw new Error('Failed to register admin user');
	adminCreds = creds;

	const page = await browser.newPage();
	await page.goto('http://localhost:5173/login');
	await page.getByRole('textbox', { name: 'Email' }).fill(adminCreds.email);
	await page.getByRole('textbox', { name: 'Password' }).fill(adminCreds.password);
	await page.getByRole('button', { name: 'Sign in', exact: true }).click();
	await page.waitForURL('**/dashboard');

	await createBranch(page, BRANCH_TO_DELETE, '123 Main St');
	await page.close();
});

test('company_manager_can_request_branch_deletion', async ({ browser }) => {
	const page = await browser.newPage();
	await page.goto('http://localhost:5173/login');
	await page.getByRole('textbox', { name: 'Email' }).fill(adminCreds.email);
	await page.getByRole('textbox', { name: 'Password' }).fill(adminCreds.password);
	await page.getByRole('button', { name: 'Sign in', exact: true }).click();
	await page.waitForURL('**/dashboard');

	await page.getByRole('link', { name: 'Branches' }).click();
	await page.waitForURL('**/branches');

	await expect(page.getByText(BRANCH_TO_DELETE)).toBeVisible();

	const deleteButton = page.getByRole('button', { name: /Delete|🗑️/i });
	await expect(deleteButton).toBeVisible();
	await deleteButton.click();

	await expect(page.getByRole('button', { name: 'Request Deletion' })).toBeVisible();
	await page.getByRole('button', { name: 'Request Deletion' }).click();

	const confirmationText = page.locator('body');
	await expect(confirmationText).toContainText('confirmation email');
});

test('branch_deletion_email_contains_confirmation_link', async ({ browser }) => {
	const page = await browser.newPage();
	await page.goto('http://localhost:5173/login');
	await page.getByRole('textbox', { name: 'Email' }).fill(adminCreds.email);
	await page.getByRole('textbox', { name: 'Password' }).fill(adminCreds.password);
	await page.getByRole('button', { name: 'Sign in', exact: true }).click();
	await page.waitForURL('**/dashboard');

	await page.getByRole('link', { name: 'Branches' }).click();
	await page.waitForURL('**/branches');

	const deleteButton = page.getByRole('button', { name: /Delete|🗑️/i });
	await deleteButton.click();
	await page.getByRole('button', { name: 'Request Deletion' }).click();

	const token = await getBranchDeletionToken(adminCreds.email);
	expect(token).toBeTruthy();
});

test('confirm_branch_deletion_via_email_link', async ({ browser }) => {
	const page = await browser.newPage();
	await page.goto('http://localhost:5173/login');
	await page.getByRole('textbox', { name: 'Email' }).fill(adminCreds.email);
	await page.getByRole('textbox', { name: 'Password' }).fill(adminCreds.password);
	await page.getByRole('button', { name: 'Sign in', exact: true }).click();
	await page.waitForURL('**/dashboard');

	await page.getByRole('link', { name: 'Branches' }).click();
	await page.waitForURL('**/branches');

	await expect(page.getByText(BRANCH_TO_DELETE)).toBeVisible();

	const deleteButton = page.getByRole('button', { name: /Delete|🗑️/i });
	await deleteButton.click();
	await page.getByRole('button', { name: 'Request Deletion' }).click();

	const token = await getBranchDeletionToken(adminCreds.email);
	expect(token).toBeTruthy();

	const confirmPage = await browser.newPage();
	await confirmPage.goto(`http://localhost:5173/confirm-branch-deletion?token=${token}`);
	await confirmPage.waitForURL('**/confirm-branch-deletion**');

	await expect(confirmPage.locator('body')).toContainText('Confirm Branch Deletion');

	await confirmPage.getByRole('button', { name: 'Delete Branch' }).click();

	await page.reload();

	await expect(page.getByText(BRANCH_TO_DELETE)).not.toBeVisible();

	await confirmPage.close();
	await page.close();
});

test('hq_staff_cannot_delete_branch', async ({ browser }) => {
	const hqEmail = `hqdelete-${Date.now()}@logsmart.app`;
	const adminPage = await browser.newPage();
	await adminPage.goto('http://localhost:5173/login');
	await adminPage.getByRole('textbox', { name: 'Email' }).fill(adminCreds.email);
	await adminPage.getByRole('textbox', { name: 'Password' }).fill(adminCreds.password);
	await adminPage.getByRole('button', { name: 'Sign in', exact: true }).click();
	await adminPage.waitForURL('**/dashboard');

	await createBranch(adminPage, 'HQ Protected Branch', '456 Protected St');

	const { sendInvitation, acceptInvitation } = await import('./utils');
	const invitationToken = await sendInvitation(browser, adminCreds, hqEmail, 'staff');
	if (!invitationToken) throw new Error('Failed to get HQ staff invitation token');
	const success = await acceptInvitation(
		await browser.newPage(),
		invitationToken,
		'HQ',
		'Delete',
		'HQDelete123!'
	);
	if (!success) throw new Error('Failed to accept HQ staff invitation');

	const hqPage = await browser.newPage();
	await hqPage.goto('http://localhost:5173/login');
	await hqPage.getByRole('textbox', { name: 'Email' }).fill(hqEmail);
	await hqPage.getByRole('textbox', { name: 'Password' }).fill('HQDelete123!');
	await hqPage.getByRole('button', { name: 'Sign in', exact: true }).click();
	await hqPage.waitForURL('**/dashboard');

	await hqPage.getByRole('link', { name: 'Branches' }).click();
	await hqPage.waitForURL('**/branches');

	await expect(hqPage.getByRole('button', { name: /Delete|🗑️/i })).not.toBeVisible();

	await adminPage.close();
	await hqPage.close();
});

test('branch_staff_cannot_delete_branch', async ({ browser }) => {
	const branchStaffEmail = `branchstaff-${Date.now()}@logsmart.app`;
	const adminPage = await browser.newPage();
	await adminPage.goto('http://localhost:5173/login');
	await adminPage.getByRole('textbox', { name: 'Email' }).fill(adminCreds.email);
	await adminPage.getByRole('textbox', { name: 'Password' }).fill(adminCreds.password);
	await adminPage.getByRole('button', { name: 'Sign in', exact: true }).click();
	await adminPage.waitForURL('**/dashboard');

	await createBranch(adminPage, 'Staff Branch', '123 Main St');

	const { sendInvitation, acceptInvitation } = await import('./utils');
	const invitationToken = await sendInvitation(
		browser,
		adminCreds,
		branchStaffEmail,
		'staff',
		'Staff Branch'
	);
	if (!invitationToken) throw new Error('Failed to get staff invitation token');
	const success = await acceptInvitation(
		await browser.newPage(),
		invitationToken,
		'Staff',
		'Branch',
		'StaffBranch123!',
		'**/logs-list'
	);
	if (!success) throw new Error('Failed to accept staff invitation');

	const staffPage = await browser.newPage();
	await staffPage.goto('http://localhost:5173/login');
	await staffPage.getByRole('textbox', { name: 'Email' }).fill(branchStaffEmail);
	await staffPage.getByRole('textbox', { name: 'Password' }).fill('StaffBranch123!');
	await staffPage.getByRole('button', { name: 'Sign in', exact: true }).click();
	await staffPage.waitForURL('**/logs-list');

	await expect(staffPage.getByRole('link', { name: 'Branches' })).not.toBeVisible();

	await adminPage.close();
	await staffPage.close();
});
