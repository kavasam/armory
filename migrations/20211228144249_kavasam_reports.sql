CREATE TABLE IF NOT EXISTS kavasam_tags (
	name TEXT NOT NULL UNIQUE,
	ID SERIAL PRIMARY KEY NOT NULL
);

CREATE TABLE IF NOT EXISTS kavasam_hashes(
	hash TEXT NOT NULL UNIQUE,
	id_type VARCHAR(32) NOT NULL,
	ID SERIAL PRIMARY KEY NOT NULL
);

CREATE TABLE IF NOT EXISTS kavasam_reports (
	hash_id INTEGER NOT NULL references kavasam_hashes(ID) ON DELETE CASCADE,
	reported_by INTEGER NOT NULL references kavasam_users(ID) ON DELETE CASCADE,
    signature TEXT NOT NULL,
	ID SERIAL PRIMARY KEY NOT NULL,
    CONSTRAINT unique_report UNIQUE(hash_id, reported_by)
);

CREATE TABLE IF NOT EXISTS kavasam_report_tags (
	tag_id INTEGER NOT NULL references kavasam_tags(ID) ON DELETE CASCADE,
	report_id INTEGER NOT NULL references kavasam_reports(ID) ON DELETE CASCADE,
    PRIMARY KEY (tag_id, report_id)
);
