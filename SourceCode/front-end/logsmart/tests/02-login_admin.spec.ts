import { test, expect, type Page } from '@playwright/test';
import { register } from './utils';

let adminCreds: {
	email: string;
	password: string;
	companyName: string;
	firstName: string;
	lastName: string;
};

test.beforeAll(async ({ browser }) => {
	const creds = await register(browser);
	if (!creds) throw new Error('Failed to register admin user');
	adminCreds = creds;
});

test('login_admin_valid', async ({ page }) => {
	await page.goto('http://localhost:5173/');
	await page.getByRole('link', { name: 'Login' }).click();
	await page.getByRole('textbox', { name: 'Email' }).click();
	await page.getByRole('textbox', { name: 'Email' }).fill(adminCreds.email);
	await page.getByRole('textbox', { name: 'Email' }).press('Tab');
	await page.getByRole('textbox', { name: 'Password' }).fill(adminCreds.password);
	await page.getByRole('button', { name: 'Sign in', exact: true }).click();
	await page.waitForURL('**/dashboard');
	await expect(page.locator('span.text-sm')).toContainText(adminCreds.email);
	await expect(page.locator('body')).toContainText(adminCreds.firstName.split('-')[0]);
	await expect(page.locator('body')).toContainText(adminCreds.email);
	await expect(page.locator('body')).toContainText(adminCreds.companyName);
	await expect(page.locator('body')).toContainText('Company Manager');
});

test('login_admin_invalid_empty_email', async ({ page }) => {
	await page.goto('http://localhost:5173/');
	await page.getByRole('link', { name: 'Login' }).click();
	await page.getByRole('textbox', { name: 'Email' }).click();
	await page.getByRole('textbox', { name: 'Email' }).press('Tab');
	await page.getByRole('textbox', { name: 'Password' }).fill('Test123!');
	await expect(page.getByRole('button', { name: 'Sign in', exact: true })).toBeDisabled();
	await expect(page).toHaveURL('/login');
});

test('login_admin_invalid_empty_password', async ({ page }) => {
	await page.goto('http://localhost:5173/');
	await page.getByRole('link', { name: 'Login' }).click();
	await page.getByRole('textbox', { name: 'Email' }).fill(adminCreds.email);
	await page.getByRole('textbox', { name: 'Email' }).press('Tab');
	await expect(page.getByRole('button', { name: 'Sign in', exact: true })).toBeDisabled();
	await expect(page).toHaveURL('/login');
});

test('login_admin_invalid_both_empty', async ({ page }) => {
	await page.goto('http://localhost:5173/');
	await page.getByRole('link', { name: 'Login' }).click();
	await expect(page.getByRole('button', { name: 'Sign in', exact: true })).toBeDisabled();
	await expect(page).toHaveURL('/login');
});

test('login_admin_invalid_email_format', async ({ page }) => {
	await page.goto('http://localhost:5173/');
	await page.getByRole('link', { name: 'Login' }).click();
	await page.getByRole('textbox', { name: 'Email' }).fill('not-an-email');
	await page.getByRole('textbox', { name: 'Email' }).press('Tab');
	await page.getByRole('textbox', { name: 'Password' }).fill('Test123!');
	await expect(page.getByRole('button', { name: 'Sign in', exact: true })).toBeDisabled();
	await expect(page).toHaveURL('/login');
});

test('login_admin_invalid_wrong_password', async ({ page }) => {
	await page.goto('http://localhost:5173/');
	await page.getByRole('link', { name: 'Login' }).click();
	await page.getByRole('textbox', { name: 'Email' }).fill(adminCreds.email);
	await page.getByRole('textbox', { name: 'Email' }).press('Tab');
	await page.getByRole('textbox', { name: 'Password' }).fill('WrongPassword123!');
	await page.getByRole('button', { name: 'Sign in', exact: true }).click();
	await expect(page.locator('body')).toContainText('Invalid email or password');
	await expect(page).toHaveURL('/login');
});

test('login_admin_invalid_nonexistent_email', async ({ page }) => {
	await page.goto('http://localhost:5173/');
	await page.getByRole('link', { name: 'Login' }).click();
	await page.getByRole('textbox', { name: 'Email' }).fill('nonexistent@logsmart.app');
	await page.getByRole('textbox', { name: 'Email' }).press('Tab');
	await page.getByRole('textbox', { name: 'Password' }).fill('Test123!');
	await page.getByRole('button', { name: 'Sign in', exact: true }).click();
	await expect(page.locator('body')).toContainText('Invalid email or password');
	await expect(page).toHaveURL('/login');
});

test('logout_admin', async ({ page }) => {
	await page.goto('http://localhost:5173/');
	await page.getByRole('link', { name: 'Login' }).click();
	await page.getByRole('textbox', { name: 'Email' }).fill(adminCreds.email);
	await page.getByRole('textbox', { name: 'Password' }).fill(adminCreds.password);
	await page.getByRole('button', { name: 'Sign in', exact: true }).click();
	await page.waitForURL('**/dashboard');
	await page.getByRole('button', { name: 'Logout' }).click();
	await page.waitForURL('**/login');
	await expect(page).toHaveURL('/login');
});

test('logout_redirects_to_login_on_protected_route_access', async ({ page }) => {
	await page.goto('http://localhost:5173/');
	await page.getByRole('link', { name: 'Login' }).click();
	await page.getByRole('textbox', { name: 'Email' }).fill(adminCreds.email);
	await page.getByRole('textbox', { name: 'Password' }).fill(adminCreds.password);
	await page.getByRole('button', { name: 'Sign in', exact: true }).click();
	await page.waitForURL('**/dashboard');
	await page.getByRole('button', { name: 'Logout' }).click();
	await page.waitForURL('**/login');
	await page.goto('http://localhost:5173/dashboard');
	await page.waitForURL('**/login');
	await expect(page).toHaveURL('/login');
});

test('unauthenticated_access_to_dashboard_redirects', async ({ page }) => {
	await page.goto('http://localhost:5173/dashboard');
	await page.waitForURL('**/login');
	await expect(page).toHaveURL('/login');
});

test('unauthenticated_access_to_users_admin_redirects', async ({ page }) => {
	await page.goto('http://localhost:5173/users-admin');
	await page.waitForURL('**/login');
	await expect(page).toHaveURL('/login');
});

test('unauthenticated_access_to_templates_dashboard_redirects', async ({ page }) => {
	await page.goto('http://localhost:5173/templates-dashboard');
	await page.waitForURL('**/login');
	await expect(page).toHaveURL('/login');
});

// OAuth helper function
async function fillMockOAuthForm(page: Page, email: string, firstName: string, lastName: string) {
	await page.fill('input[name="subject"], input#subject, input[type="text"]', email);
	const claimsJson = JSON.stringify({
		email: email,
		email_verified: true,
		given_name: firstName,
		family_name: lastName,
		picture: 'https://example.com/avatar.jpg'
	});
	await page.fill('textarea', claimsJson);
	await page
		.getByRole('button', { name: /sign-in/i })
		.first()
		.click();
}

test.describe('Google OAuth Authentication', () => {
	let oauthAdminCreds: {
		email: string;
		password: string;
		firstName: string;
		lastName: string;
	};

	test.beforeAll(async ({ browser }) => {
		const creds = await register(browser);
		if (!creds) throw new Error('Failed to register admin user');
		oauthAdminCreds = creds;
	});

	test('oauth_google_link_and_login', async ({ page }) => {
		test.skip(!!process.env.CI, 'Skipping Google OAuth test on CI due to potential flakiness');
		await page.goto('http://localhost:5173/login');
		await page.getByRole('textbox', { name: 'Email' }).fill(oauthAdminCreds.email);
		await page.getByRole('textbox', { name: 'Password' }).fill(oauthAdminCreds.password);
		await page.getByRole('button', { name: 'Sign in', exact: true }).click();
		await page.waitForURL('**/dashboard');

		await page.goto('http://localhost:5173/settings');
		await page.waitForLoadState('networkidle');
		await page.getByRole('button', { name: 'Link Google Account' }).click();
		await page.waitForURL(/localhost:8080/);

		await fillMockOAuthForm(
			page,
			oauthAdminCreds.email,
			oauthAdminCreds.firstName,
			oauthAdminCreds.lastName
		);
		await page.waitForURL('**/settings');
		await expect(page.getByRole('button', { name: /unlink google account/i })).toBeVisible();
		await page.getByRole('button', { name: /logout/i }).click();
		await page.waitForURL('**/login');

		await page.getByRole('button', { name: 'Sign in with Google' }).click();
		await page.waitForURL(/localhost:8080/);

		await fillMockOAuthForm(
			page,
			oauthAdminCreds.email,
			oauthAdminCreds.firstName,
			oauthAdminCreds.lastName
		);
		await page.waitForURL('**/dashboard');
		await expect(page.locator('body')).toContainText(oauthAdminCreds.email);
	});

	test('oauth_google_unlink_and_attempt_signin', async ({ page }) => {
		test.skip(!!process.env.CI, 'Skipping Google OAuth test on CI due to potential flakiness');
		await page.goto('http://localhost:5173/login');
		await page.getByRole('textbox', { name: 'Email' }).fill(oauthAdminCreds.email);
		await page.getByRole('textbox', { name: 'Password' }).fill(oauthAdminCreds.password);
		await page.getByRole('button', { name: 'Sign in', exact: true }).click();
		await page.waitForURL('**/dashboard');

		await page.goto('http://localhost:5173/settings');
		await page.waitForLoadState('networkidle');
		await page.getByRole('button', { name: 'Link Google Account' }).click();
		await page.waitForURL(/localhost:8080/);

		await fillMockOAuthForm(
			page,
			oauthAdminCreds.email,
			oauthAdminCreds.firstName,
			oauthAdminCreds.lastName
		);
		await page.waitForURL('**/settings');
		await expect(page.getByRole('button', { name: /unlink google account/i })).toBeVisible();

		await page.getByRole('button', { name: 'Unlink Google Account' }).click();
		await page.getByRole('button', { name: 'Confirm' }).click();

		await page.waitForLoadState('networkidle');
		await expect(page.locator('body')).toContainText(/Link Google Account/i);
		await expect(page.locator('body')).not.toContainText(/google account is linked/i);

		await page.getByRole('button', { name: /logout/i }).click();
		await page.waitForURL('**/login');

		await page.getByRole('button', { name: 'Sign in with Google' }).click();
		await page.waitForURL(/localhost:8080/);

		await fillMockOAuthForm(
			page,
			oauthAdminCreds.email,
			oauthAdminCreds.firstName,
			oauthAdminCreds.lastName
		);
		await expect(page.url()).toContain(
			'http://localhost:5173/login?oauth_error=An%20account%20with%20this%20email%20already%20exists'
		);
	});
});
