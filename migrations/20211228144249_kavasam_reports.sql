CREATE TABLE IF NOT EXISTS kavasam_hashes(
	hash TEXT NOT NULL UNIQUE,
	id_type VARCHAR(32) NOT NULL,
	ID SERIAL PRIMARY KEY NOT NULL
);

CREATE TABLE IF NOT EXISTS kavasam_reports (
	hash_id INTEGER NOT NULL references kavasam_hashes(ID) ON DELETE CASCADE,
	reported_by INTEGER NOT NULL references kavasam_users(ID) ON DELETE CASCADE,
    PRIMARY KEY (hash_id, reported_by)
);
