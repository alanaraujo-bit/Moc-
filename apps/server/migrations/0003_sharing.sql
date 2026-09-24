-- Shared vaults. The vault and its items stay in the owner's account; members get a
-- grant (vault key sealed to them, signed by the owner) and access by membership.
CREATE TABLE vault_members (
    owner_id   UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    vault_id   UUID NOT NULL,
    member_id  UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    role       TEXT NOT NULL CHECK (role IN ('editor', 'reader')),
    key_gen    INTEGER NOT NULL,        -- writes must use at least this key generation
    grant_json TEXT NOT NULL,           -- signed ShareGrant (opaque to the server)
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (owner_id, vault_id, member_id)
);
CREATE INDEX vault_members_member ON vault_members(member_id);
