# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Common Commands

*   **Build**: `cargo build`
*   **Run**: `cargo run`
*   **Test**: `cargo test`
*   **Lint**: `cargo clippy`
*   **Check for errors**: `cargo check`

## Architecture

This project is a Rust-based AI API management system built with the `axum` framework. It follows a modular architecture:

*   `src/main.rs`: The application entry point.
*   `src/lib.rs`: Optional library part.
*   `src/config`: Application configuration.
*   `src/routes`: Defines API and web routes.
    *   `api.rs`: API-specific routes.
    *   `web.rs`: Web-specific routes.
*   `src/handlers`: Contains request handlers for different routes.
*   `src/models`: Data models for database interaction.
*   `src/services`: Business logic and services.
*   `src/database`: Database connection and interaction logic (using `sqlx` with SQLite).
*   `src/middlewares`: Custom middleware for requests.
*   `src/errors`: Custom error handling.
*   `src/utils`: Utility functions.
*   `migrations`: Database migration files.
*   `tests`: Integration tests.
*   `Cargo.toml`: Project dependencies and metadata.

## General

* to memorize
