---
name: Jobtowel
description: A quiet, warm workspace for moving a job search forward.
colors:
  primary: "#b94332"
  success: "#347453"
  danger: "#b73535"
  warning: "#a36a13"
  body: "#302d2a"
  canvas: "#fffefa"
  muted: "#716b65"
  surface: "#f7f5f1"
  selected: "#f9e9e3"
  border: "#e7e2db"
  input-border: "#d8d1c9"
  placeholder: "#78716a"
  navigation-hover: "#eeeae4"
  navigation-accent: "#a43b2c"
  outline-accent: "#8c372b"
  profile-fill: "#eae2d7"
  profile-text: "#5f5143"
  person-fill: "#eee8df"
  person-text: "#665442"
  person-hover: "#e6dacc"
typography:
  title:
    fontFamily: 'system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", "Noto Sans", "Liberation Sans", Arial, sans-serif, "Apple Color Emoji", "Segoe UI Emoji", "Segoe UI Symbol", "Noto Color Emoji"'
    fontSize: "1.85rem"
    fontWeight: 650
    lineHeight: 1.2
    letterSpacing: "-.025em"
  body:
    fontFamily: 'system-ui, -apple-system, "Segoe UI", Roboto, "Helvetica Neue", "Noto Sans", "Liberation Sans", Arial, sans-serif, "Apple Color Emoji", "Segoe UI Emoji", "Segoe UI Symbol", "Noto Color Emoji"'
    fontSize: ".9375rem"
    fontWeight: 400
    lineHeight: 1.5
  label:
    fontSize: ".875rem"
    fontWeight: 600
  compact:
    fontSize: ".8125rem"
  brand:
    fontSize: "1.2rem"
    fontWeight: 700
    letterSpacing: "-.035em"
rounded:
  control: ".6rem"
  panel: ".85rem"
  filter: ".5rem"
  avatar: "50%"
spacing:
  compact: ".75rem"
  default: "1rem"
  panel: "1.5rem"
  form: "2rem"
  heading-gap: "2.5rem"
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "#ffffff"
    rounded: "{rounded.control}"
    padding: ".65rem 1rem"
  button-outline-primary:
    backgroundColor: "transparent"
    textColor: "{colors.outline-accent}"
    rounded: "{rounded.control}"
    padding: ".65rem 1rem"
  button-outline-primary-hover:
    backgroundColor: "{colors.selected}"
    textColor: "{colors.outline-accent}"
  input:
    backgroundColor: "{colors.canvas}"
    textColor: "{colors.body}"
    rounded: "{rounded.control}"
    padding: ".7rem .85rem"
  navigation-link:
    textColor: "{colors.body}"
    rounded: "{rounded.control}"
    padding: ".75rem .85rem"
  navigation-link-selected:
    backgroundColor: "{colors.selected}"
    textColor: "{colors.navigation-accent}"
  summary-panel:
    backgroundColor: "{colors.surface}"
    rounded: "{rounded.panel}"
    padding: "{spacing.panel}"
  profile-trigger:
    backgroundColor: "{colors.profile-fill}"
    textColor: "{colors.profile-text}"
    rounded: "{rounded.avatar}"
    width: "2.75rem"
    height: "2.75rem"
  person-avatar:
    backgroundColor: "{colors.person-fill}"
    textColor: "{colors.person-text}"
    rounded: "{rounded.avatar}"
    width: "2.75rem"
    height: "2.75rem"
---

# Design System: Jobtowel

## Overview

Jobtowel is a quiet, warm workspace for a job search. The user chose a clean Todoist-inspired direction: cream surfaces, a coral primary action, system typography, and small interactions that acknowledge progress. Keep Bootstrap 5.3, server-rendered Maud views, and HTMX as the foundation.

The workspace favors clear task hierarchy and readable records. A desktop sidebar anchors navigation; on phones it becomes a compact sticky header. Summary panels use soft tonal backgrounds, milestone days use light dividers, and application rows stack into readable records on small screens.

Key characteristics:

- Warm light surfaces with restrained coral emphasis.
- Native controls, visible labels, and clear keyboard focus.
- Compact supporting information with room around primary actions.
- Brief state feedback with reduced-motion support.

The normative tokens above come from `frontend/scss/app.scss`, `frontend/scss/components/table.scss`, and Bootstrap defaults. The extensions sidecar records motion, breakpoints, depth, and component previews.

## Colors

The warm neutral palette keeps job information central. Coral identifies primary actions and selected navigation; semantic success, danger, and warning colors retain their conventional meaning.

### Primary

Use `primary` for primary buttons, links, focus outlines, and completed timeline dots. Selected navigation uses the pale `selected` surface and darker `navigation-accent` text. Secondary outlined actions use `outline-accent` with `input-border` borders.

### Neutral

Use `canvas` for the page and inputs, `surface` for the sidebar, summary panels, authentication panel, and empty states, and `border` for dividers. `body` is the default text color; `muted` carries supporting text. Warm profile and person colors distinguish avatar controls without adding another accent system.

## Typography

Use Bootstrap's locally available system sans-serif stack. No remote font is required. Body text is regular; labels and action text are semibold; page headings have tighter tracking. Heading, brand, label, and compact values are recorded above.

Page titles reduce to 1.55rem below the mobile breakpoint. Dashboard summary labels use the label size; milestone dates, timeline dates, and table headings use the compact size. Numeric application counts use tabular figures. Note typography remains scoped to the Markdown content region with a 75ch maximum width.

## Layout

Signed-in desktop pages use a fixed sidebar (15rem) and a main region capped at 80rem, padded 3.25rem vertically and 3.5rem horizontally. Below 1200px the sidebar becomes 13rem and main padding becomes 2.5rem by 2rem. At 1600px and above the main region shifts toward the center of the available workspace.

Below 768px the sidebar becomes a full-width sticky header with the brand and profile on the first row and Home/Applied on the second. Hide the sidebar's extra recording shortcut and profile text; retain the page's main recording action. Main padding becomes 1.75rem by 1.1rem. Non-small buttons have a minimum 44px height on mobile.

Authentication forms use a centered responsive panel capped at 30rem with 2rem padding, reduced to 1.5rem on phones. Account forms stay within the same width with a transparent surface in the workspace. Application, event, and person forms keep the 42rem maximum width.

Dashboard summary panels stack on phones. Milestone days remain a dated list separated by borders; empty days have smaller padding, and on phones their date and “No events.” stay side by side. Applied retains desktop sorting and filtering and presents stacked records on phones, keeping role and company filters visible, status beside the role, movement below, and actions at the bottom.

## Elevation & Depth

Use tonal backgrounds and light dividers for ordinary page structure. Summary panels and empty states sit flat. Profile and person popovers use the shared warm shadow (`0 8px 28px rgba(48, 45, 42, .14)`) to separate temporary content from the page. Timeline dots have a canvas-colored halo so connecting lines do not cross them.

## Shapes

Controls and navigation links use the control radius; summary, authentication, and empty-state panels use the panel radius. Profile and people controls are circular. Borders are quiet and warm. Inputs stay native browser controls with conventional Bootstrap styling, rather than simulated controls.

## Components

### Buttons and fields

Primary actions use coral with white text and semibold labels. Secondary actions use warm outlined buttons; destructive actions retain danger styling. Pressing a button scales it to .97. Buttons and fields transition for 180ms; buttons use the shared easing curve for transforms. Field hover darkens the border, and focus retains Bootstrap's visible primary ring. Links, buttons, and summary controls have a visible 3px primary outline with 3px offset.

### Navigation and menus

Navigation links combine locally rendered stroke icons with text. Mark the selected destination with `aria-current="page"`, a pale coral background, and darker semibold text. The profile control shows uppercase initials from the first and last names, or one initial for a single name, and opens Account and Log Out. Use native details menus with keyboard focus, Escape dismissal, and click-outside dismissal. Profile menus open above the control in the desktop sidebar and below it on mobile.

### Panels, milestones, and empty states

Summary panels use tonal backgrounds and restrained labels; milestone days use dividers rather than individual cards. Empty states combine a small local icon, a clear heading, brief guidance, and a relevant action. Contact cards use the shared popover shadow and fit the viewport.

### Feedback and motion

Menus reveal over 180ms with a small opacity, translation, and scale change. Person avatars lift 2px on hover or while open. Success alerts use a 320ms background acknowledgment; successful fragment updates use 240ms. Disable animations and transitions and restore automatic scrolling when `prefers-reduced-motion: reduce` is set.

### Form, application, event, and note behavior

Server and network errors appear near the form without clearing draft text.

Auth forms use inline server validation returned by HTMX, preserve non-password drafts, and leave passwords blank after errors. Account updates show a success message after redirect.

Applied uses a responsive Bootstrap Table with sortable Role and Company columns and labeled text filters for each, a Movement column showing the latest event title by event date with a smaller date and time subtitle in the profile timezone (or “No events”), alongside Edit/Delete actions. A Record new application button opens the shared create/edit form, capped at 42rem. Role forms preserve every draft field on HTMX validation errors. Deletion uses native confirmation with HTMX table replacement and a separate confirmation page as the JavaScript-free fallback. Applied and Home share the workspace navigation.

Event forms use a title dropdown with interview, take home, offer, and recruiter screen stages plus Custom. Selecting Custom reveals a required custom title input; without JavaScript the input stays visible with guidance. Existing titles outside the preset list open as Custom.

Timeline events after the current instant use a dotted connecting line and a warning-emphasis dot. Past events keep the solid line and primary dot.

Timeline people appear as circular avatars with uppercase first and last initials (one initial when the last name is absent). Hover, keyboard focus, or a tap opens a contact card with first and last names on separate lines, an optional job title, and phone/email icon links only when those fields are set. Keep person notes in the card. A top-right Update link opens a prefilled person form using the existing 42rem form layout, with required first name and optional last name, job title, email, phone, and notes. Explain that changes apply to every linked event in the application; Save changes and Cancel return to its timeline. Preserve drafts during HTMX validation with a separate request-error region and normal form fallbacks. Cards stay reachable as the pointer moves from the avatar, fit the viewport, and dismiss on Escape or outside click. Native details provides the no-JavaScript fallback.

Each event person row uses a labeled native select for people from the same application, defaulting to Add a new person. Existing options show the person's name and email when present. Selecting an existing person hides and disables the new-person fields; selecting Add a new person reveals the labeled fields with required first name and optional last name, job title, email, phone, and notes. Without JavaScript, those fields remain visible with guidance to fill them only when adding a new person. Keep rows in bordered fieldsets with numbered legends and a Remove person action; adding or removing rows preserves the event draft.

Applications default to active. The edit form uses a labeled Active application switch. Applied has an Application status filter (Active by default, Inactive, or All applications), a status column with smaller muted “X days” subtext for active applications (singular “1 day”), counted from the earliest event date through today in the profile timezone, and a GET form with HTMX table replacement and a no-JavaScript fallback. Home counts active applications. Creating a Removed from consideration event asks whether to mark the application inactive or keep its current status before saving the event and any status change together.

Only Account shows a labeled native timezone select. Signup detects the browser timezone through a hidden field; Account shows the saved choice. All displayed dates and times, including tooltips, use “Month DD, YYYY at H:MM AM/PM” (for example, “March 23, 2026 at 9:30 AM”) in the profile timezone, with no visible timezone or offset outside Account. Date inputs use native browser controls. Event forms reject nonexistent daylight saving times and offer a first/second occurrence select for repeated times, preserving the saved occurrence when editing. Browser detection only initializes unset profiles and refreshes date displays when no form draft has been changed.

Job descriptions and event/person notes use a shared Markdown editor with Bold, Italic, Heading, List, Quote, Link, and Code buttons and an on-demand preview. Keep the native textarea as the submitted value and preserve Markdown source on edits and validation. Hide toolbar controls without JavaScript. Previews and all saved-note displays use the same sanitized server renderer; notes cannot inject scripts, application attributes, styles, or embedded media. Scope typography to `.notes-content`: the largest note heading is 1.25em, smaller headings step down, and spacing, lists, quotes, code, and scrolling tables fit timelines and contact cards.

## Do's and Don'ts

- Do preserve the warm surfaces, coral primary action, system typography, and responsive workspace navigation.
- Do keep real labels, native form controls, keyboard focus, normal form fallbacks, and server validation.
- Do keep feedback brief and honor reduced-motion preferences.
- Do preserve functional conventions above when extending the visual system.
- Don't require JavaScript for basic form submission or access to native details menus.
- Don't introduce remote font or icon dependencies for this interface.
- Don't use motion or color as the only indication of state or an error.

## Color modes

Account offers a labeled native Theme select with Auto, Light, and Dark, saved with the profile. Auto is the default for existing and new accounts and follows `prefers-color-scheme` through CSS, including live changes and no-JavaScript visits. Signed-out pages use Auto. The server renders the saved `data-bs-theme` before styles load to avoid a light flash. Bootstrap’s shared color-mode mixin emits both explicit Dark and Auto media-query variants, covering native fields and Bootstrap components.

Dark keeps the warm identity: canvas `#211f1d`, surface `#2a2724`, text `#e8e2db`, muted text `#b9b0a7`, borders `#514940`, selected navigation `#482e27`, and coral links `#f09a87`. Navigation, avatars, table hover, outlines, popovers, Markdown, and feedback use theme variables. Preserve the existing light palette and layout in Light mode.
