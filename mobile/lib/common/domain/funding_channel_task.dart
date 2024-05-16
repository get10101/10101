import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;

enum FundingChannelTaskStatus {
  pending,
  funded,
  orderCreated,
  failed;

  static (FundingChannelTaskStatus, String?) fromApi(dynamic taskStatus) {
    if (taskStatus is bridge.FundingChannelTask_Pending) {
      return (FundingChannelTaskStatus.pending, null);
    }

    if (taskStatus is bridge.FundingChannelTask_Funded) {
      return (FundingChannelTaskStatus.funded, null);
    }

    if (taskStatus is bridge.FundingChannelTask_Failed) {
      final error = taskStatus.field0;
      return (FundingChannelTaskStatus.failed, error);
    }

    if (taskStatus is bridge.FundingChannelTask_OrderCreated) {
      final orderId = taskStatus.field0;
      return (FundingChannelTaskStatus.orderCreated, orderId);
    }

    return (FundingChannelTaskStatus.pending, null);
  }

  static bridge.FundingChannelTask apiDummy() {
    return const bridge.FundingChannelTask_Pending();
  }

  @override
  String toString() {
    switch (this) {
      case FundingChannelTaskStatus.pending:
        return "Pending";
      case FundingChannelTaskStatus.failed:
        return "Failed";
      case FundingChannelTaskStatus.funded:
        return "Funded";
      case FundingChannelTaskStatus.orderCreated:
        return "OrderCreated";
    }
  }
}
