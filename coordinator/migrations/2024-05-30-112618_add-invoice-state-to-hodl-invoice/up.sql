CREATE TYPE "InvoiceState_Type" AS ENUM ('Open', 'Accepted', 'Settled', 'Failed');

ALTER TABLE "hodl_invoices"
    ADD COLUMN "invoice_state" "InvoiceState_Type" NOT NULL DEFAULT 'Open',
    ADD COLUMN "order_id" UUID;
