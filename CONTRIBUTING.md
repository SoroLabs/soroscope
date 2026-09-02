# Contributing to SoroScope

Thank you for your interest in contributing to *SoroScope*! We are excited to have you as part of our community.

As a project in the *Stellar Wave Program*, we value collaboration and clear communication. Please use the following guides to help you get started:

## 💈 Guides

- [**Development & Setup**)]./docs/development.md): How to set up the monorepo and our coding standards.
- [**How to Open a Pull Request**]./docs/pull-requests.md): Our step-by-step workflow for submitting code.
- [**Reporting Issues**]./docs/issues.md): How to report bugs or suggest new features.

## 📵 Mobile Viewport Checklist

To prevent horizontal scroll overflow on mobile screen widths, follow these steps when making changes to responsive layouts:

1. Add `max-w-full overflow-x-auto` to resource data table containers.
2. Wrap responsive flex grids with proper breakpoint utility classes.
3. Test layout at 375px mobile breakpoint.

### Acceptance Criteria

- Inspect the site using mobile view mode in DevTools.
- Confirm zero horizontal body scrollbar.

## 💅 Questions?
Feel free to open an **Issue**or reach out to the *SoroLabs team. Let's build the best Soroban developer tools together!