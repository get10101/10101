ALTER TYPE "Message_Type_Type"
    ADD
    VALUE IF NOT EXISTS 'RolloverAccept';
ALTER TYPE "Message_Type_Type"
    ADD
    VALUE IF NOT EXISTS 'RolloverConfirm';
ALTER TYPE "Message_Type_Type"
    ADD
    VALUE IF NOT EXISTS 'RolloverFinalize';
ALTER TYPE "Message_Type_Type"
    ADD
    VALUE IF NOT EXISTS 'RolloverRevoke';
