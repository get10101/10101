## Database development

For the development, we just use tmp to create a db.
This is needed so that we can generate the db schema files.

```bash
DATABASE_URL="sqlite:///tmp/database.sql" diesel setup
```

```bash
DATABASE_URL="sqlite:///tmp/database.sql" diesel migration run
```
