# Introduction

superui is a Bevy plugin that provides a browser-like environment for running
HTML/CSS/JS applications — and Solid-style `.tsx` components via the supersolid
framework — as game UI. It is built on top of `bevy_ui` (inheriting some of its
limitations for now) and a modified `bevy_flair` for CSS support.

The goal is the best possible developer experience for writing game UI in Bevy:
rapid iteration (hot reload) and compatibility with existing web-development
knowledge.

## Status

This is in very early development. Some working examples already run — see the
[gallery](../examples/). The code is largely AI-generated and not yet fully
reviewed; APIs are expected to be in flux, though the surface deliberately
mirrors familiar web APIs. Use at your own risk.
