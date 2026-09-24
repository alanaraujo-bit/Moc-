-- Encrypted attachment blobs (sealed on the device, bound to item + attachment ids).
CREATE TABLE attachments (
    account_id UUID NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    id         UUID NOT NULL,
    item_id    UUID NOT NULL,
    blob       BYTEA NOT NULL,
    size       BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (account_id, id)
);
CREATE INDEX attachments_item ON attachments(account_id, item_id);
