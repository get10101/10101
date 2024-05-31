-- Postgres does not allow removing enum type values. One can only re-create an enum type with fewer values and replace the references.
-- However, there is no proper way to replace the values to be removed where they are used (i.e. referenced in `hodl_invoice` table)
-- We opt to NOT remove enum values that were added at a later point.

select 1;
