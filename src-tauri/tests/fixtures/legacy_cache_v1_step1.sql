-- schema_version=1 相当（sort_order 追加済み、row_number 未追加）の最小 fixture。
CREATE TABLE sources (
    id INTEGER PRIMARY KEY, type TEXT NOT NULL, name TEXT NOT NULL,
    config_json TEXT NOT NULL, refresh_interval INTEGER NOT NULL DEFAULT 3600,
    status TEXT, last_success_at TEXT, last_error TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE records (
    id INTEGER PRIMARY KEY, source_id INTEGER NOT NULL, label TEXT,
    open_ref TEXT, fields_json TEXT NOT NULL, search_text TEXT NOT NULL
);
CREATE TABLE app_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
INSERT INTO app_meta(key, value) VALUES('schema_version', '1');
INSERT INTO sources(id, type, name, config_json, refresh_interval, sort_order)
  VALUES(8, 'csv', '中間版ソース', '{"path":"C:\\mid.csv"}', 900, 3);
INSERT INTO records(source_id, label, open_ref, fields_json, search_text)
  VALUES(8, '中間版レコード', NULL, '{"name":"中間版"}', '中間版');
