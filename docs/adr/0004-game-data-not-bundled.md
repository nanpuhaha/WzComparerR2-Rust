# Game data is never bundled; `WZ_DATA_ROOT` is the only data seam

The repository ships no WZ files, images, sounds, or fonts. Tools that need real game
data read the user's local `Data` directory through the `WZ_DATA_ROOT` environment
variable; tests that require it skip cleanly when it is unset. This keeps the
repository publishable and portable, and avoids redistributing copyrighted assets.

**Consequences**: CI uses the synthetic corpus for most coverage. Real-WZ integration
tests only run on a machine with a legitimate game installation.
