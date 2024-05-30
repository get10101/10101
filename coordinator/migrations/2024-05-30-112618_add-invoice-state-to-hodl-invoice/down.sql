ALTER TABLE "hodl_invoices"
    DROP COLUMN "invoice_state",
    DROP COLUMN "order_id";

DROP TYPE "InvoiceState_Type";
