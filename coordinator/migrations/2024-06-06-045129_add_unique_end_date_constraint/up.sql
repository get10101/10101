ALTER TABLE funding_rates
ADD CONSTRAINT unique_end_date UNIQUE (end_date);
