-- Mocó sync server. Everything user-authored is ciphertext produced on the devices.

CREATE TABLE accounts (
    id              UUID PRIMARY KEY,
    email           TEXT NOT NULL UNIQUE,
    record          TEXT NOT NULL,          -- encrypted account record (JSON of ciphertexts)
    record_version  BIGINT NOT NULL DEFAULT 1,
    auth_public_key BYTEA NOT NULL,         -- Ed25519, verifies login signatures
    seq             BIGINT NOT NULL DEFAULT 0,
    totp_secret     BYTEA,                  -- encrypted with the server key
    totp_recovery   TEXT[] NOT NULL DEFAULT '{}', -- SHA-256 of unused recovery codes
    plan            TEXT NOT NULL DEFAULT 'pessoal',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE devices (
    id           UUID PRIMARY KEY,
    account_id   UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    platform     TEXT NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at   TIMESTAMPTZ
);
CREATE INDEX devices_account ON devices(account_id);

CREATE TABLE sessions (
    token_hash   BYTEA PRIMARY KEY,
    account_id   UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    device_id    UUID NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at   TIMESTAMPTZ NOT NULL
);
CREATE INDEX sessions_account ON sessions(account_id);

CREATE TABLE challenges (
    nonce      BYTEA PRIMARY KEY,
    account_id UUID NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE vaults (
    account_id  UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    id          UUID NOT NULL,
    version     BIGINT NOT NULL,
    wrapped_key BYTEA NOT NULL,
    attrs       BYTEA NOT NULL,
    deleted     BOOLEAN NOT NULL,
    created_at  BIGINT NOT NULL,
    updated_at  BIGINT NOT NULL,
    seq         BIGINT NOT NULL,
    PRIMARY KEY (account_id, id)
);
CREATE INDEX vaults_seq ON vaults(account_id, seq);

CREATE TABLE items (
    account_id  UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    id          UUID NOT NULL,
    vault_id    UUID NOT NULL,
    version     BIGINT NOT NULL,
    overview    BYTEA NOT NULL,
    details     BYTEA NOT NULL,
    deleted     BOOLEAN NOT NULL,
    created_at  BIGINT NOT NULL,
    updated_at  BIGINT NOT NULL,
    seq         BIGINT NOT NULL,
    PRIMARY KEY (account_id, id)
);
CREATE INDEX items_seq ON items(account_id, seq);

CREATE TABLE audit (
    id         BIGSERIAL PRIMARY KEY,
    account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    kind       TEXT NOT NULL,
    device_id  UUID
);
CREATE INDEX audit_account ON audit(account_id, at DESC);
