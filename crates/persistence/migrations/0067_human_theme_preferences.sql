ALTER TABLE human_accounts
    ADD COLUMN theme_mode TEXT NOT NULL DEFAULT 'system'
        CHECK (theme_mode IN ('system', 'light', 'dark')),
    ADD COLUMN light_theme TEXT NOT NULL DEFAULT 'catppuccin-latte',
    ADD COLUMN dark_theme TEXT NOT NULL DEFAULT 'default';
