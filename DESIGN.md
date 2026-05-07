---
version: alpha
name: Buildbelt by NautaAI
description: Apple-adjacent local AI setup companion with Holster safety engine.
colors:
  primary: "#1A1A1F"
  on-primary: "#FFFFFF"
  secondary: "#2F7A4A"
  on-secondary: "#FFFFFF"
  accent: "#B8781F"
  on-accent: "#1A1A1F"
  success: "#2F7A4A"
  danger: "#B04A30"
  background: "#F5F3ED"
  surface: "#FFFFFF"
  surface-muted: "#ECE7DC"
  surface-tint: "#F3FBF5"
  border: "#D8D3C4"
  border-strong: "#C2BCAE"
  text: "#1A1A1F"
  text-muted: "#6C6C78"
  text-subtle: "#85808A"
typography:
  display:
    fontFamily: "-apple-system, BlinkMacSystemFont, SF Pro Display, SF Pro Text, Helvetica Neue, sans-serif"
    fontSize: "28px"
    fontWeight: 750
    lineHeight: "1.06"
    letterSpacing: "0"
  title:
    fontFamily: "-apple-system, BlinkMacSystemFont, SF Pro Text, Helvetica Neue, sans-serif"
    fontSize: "20px"
    fontWeight: 700
    lineHeight: "1.15"
    letterSpacing: "0"
  body:
    fontFamily: "-apple-system, BlinkMacSystemFont, SF Pro Text, Helvetica Neue, sans-serif"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: "1.45"
    letterSpacing: "0"
  caption:
    fontFamily: "-apple-system, BlinkMacSystemFont, SF Pro Text, Helvetica Neue, sans-serif"
    fontSize: "12px"
    fontWeight: 600
    lineHeight: "1.35"
    letterSpacing: "0.04em"
rounded:
  xs: "6px"
  sm: "8px"
  md: "10px"
  lg: "14px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "12px"
  lg: "16px"
  xl: "24px"
  xxl: "32px"
components:
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.on-accent}"
    rounded: "{rounded.xs}"
    padding: "8px 14px"
  button-secondary:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.xs}"
    padding: "8px 14px"
  card:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.sm}"
    padding: "{spacing.lg}"
  safety-card:
    backgroundColor: "{colors.surface-tint}"
    textColor: "{colors.text}"
    rounded: "{rounded.md}"
    padding: "{spacing.lg}"
---

## Overview

Buildbelt by NautaAI is a premium local setup companion for people and small teams entering AI safely. It should feel calm, expensive, and trustworthy, closer to a native Apple utility than a loud SaaS dashboard.

Holster is the safety engine inside Buildbelt. Buildbelt explains the path; Holster protects keys, project handoff, local scans, and runtime safety.

The product should create a small "wow" moment through clarity, polish, and motion restraint: the user should feel guided, not sold to.

## Colors

Use warm off-white surfaces, precise borders, deep charcoal text, green safety signals, and restrained amber primary actions.

Green means local safety, readiness, approved path, and protected handoff. Amber means primary action or attention. Red is only for danger or destructive action.

Avoid one-note palettes. Do not let the app become all green, all amber, all beige, or all dark slate. Use color as signal, not decoration.

## Typography

Use Apple system fonts. Keep text crisp, direct, and human. Headlines should be confident but not oversized inside modals or tool surfaces.

Use uppercase captions only for compact metadata labels such as "Buildbelt recommendation", "Doctor timing", and "Holster inside". Letter spacing should remain positive or zero, never negative.

## Layout & Spacing

Prefer generous spacing, clean alignment, and shallow hierarchy. The user should scan the page in calm bands:

1. Brand and promise.
2. Audience mode.
3. Setup path.
4. Recommendation or lesson.
5. Next action.

Use cards for individual decisions, lessons, and summaries. Do not nest decorative cards inside decorative cards. Avoid marketing hero layouts inside the desktop app.

## Elevation & Depth

Depth should be subtle: soft modal shadow, thin borders, and small surface shifts. Avoid heavy shadows, glassmorphism, or floating ornament.

## Shapes

Default radius is 6px to 10px. Larger radii are allowed only for branded marks, segmented controls, and modal containers. Avoid pill-heavy UI unless the element is a true badge or progress marker.

## Components

Buttons should feel native and quiet. Primary buttons use amber and should appear only for meaningful next steps. Secondary and ghost buttons should stay visually calm.

Segmented controls are preferred for mode switches like Personal/Business. Cards should expose real decisions or statuses, not generic decoration.

The NautaAI mark should be simple and confident. Until a final logo exists, use a clean "N" mark in a rounded square with green safety tint.

## Do's and Don'ts

Do:
- Make the first screen immediately useful.
- Keep all copy plain-language and beginner-safe.
- Show the user's next best move.
- Preserve local-first trust.
- Make Personal and Business feel like two modes of the same product.
- Verify desktop and mobile screenshots after UI changes.

Don't:
- Use loud gradients, decorative blobs, or generic AI imagery.
- Hide the product behind a marketing landing page.
- Overload beginners with provider jargon.
- Make API keys feel casual.
- Use public-posting, cloud-sync, or deploy language unless explicitly approved.
