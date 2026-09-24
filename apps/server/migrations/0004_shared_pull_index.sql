-- Shared pulls read one vault's items by sequence number.
CREATE INDEX items_vault_seq ON items(account_id, vault_id, seq);
