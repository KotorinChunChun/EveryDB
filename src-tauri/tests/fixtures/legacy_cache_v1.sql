-- v0.0.1〜v0.0.5移行前の最小cache DB fixture。実在の認証情報・個人情報は含めない。
CREATE TABLE sources (
    id INTEGER PRIMARY KEY, type TEXT NOT NULL, name TEXT NOT NULL,
    config_json TEXT NOT NULL, refresh_interval INTEGER NOT NULL DEFAULT 3600,
    status TEXT, last_success_at TEXT, last_error TEXT
);
CREATE TABLE records (
    id INTEGER PRIMARY KEY, source_id INTEGER NOT NULL, label TEXT,
    open_ref TEXT, fields_json TEXT NOT NULL, search_text TEXT NOT NULL
);
INSERT INTO sources(id, type, name, config_json, refresh_interval)
  VALUES(7, 'csv', '移行前ソース', '{"path":"C:\\legacy.csv"}', 900);
INSERT INTO records(source_id, label, open_ref, fields_json, search_text)
  VALUES(7, '移行前レコード', 'file:///C:/legacy.csv', '{"name":"移行前"}', '移行前');