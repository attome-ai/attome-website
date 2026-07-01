CREATE TABLE IF NOT EXISTS core_apikey (
    id           UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    user_ref     UUID        NOT NULL REFERENCES core_systemuser(id) ON DELETE CASCADE,
    name         TEXT        NOT NULL,
    public_key   TEXT        NOT NULL,
    secret_hash  TEXT        NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at TIMESTAMPTZ,
    is_active    BOOL        NOT NULL DEFAULT TRUE,

    CONSTRAINT core_apikey_public_key_unique UNIQUE (public_key)
);

CREATE INDEX IF NOT EXISTS core_apikey_user_ref_idx   ON core_apikey (user_ref);
CREATE INDEX IF NOT EXISTS core_apikey_public_key_idx ON core_apikey (public_key);
