ALTER TYPE "OrderReason_Type"
    ADD
    VALUE IF NOT EXISTS 'CoordinatorLiquidated';

ALTER TYPE "OrderReason_Type"
    ADD
    VALUE IF NOT EXISTS 'TraderLiquidated';
