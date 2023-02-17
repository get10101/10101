import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;

enum OrderState {
  open,
  filled,
  failed;

  static OrderState fromApi(bridge.OrderState orderState) {
    switch (orderState) {
      case bridge.OrderState.Open:
        return OrderState.open;
      case bridge.OrderState.Filled:
        return OrderState.filled;
      case bridge.OrderState.Failed:
        return OrderState.failed;
    }
  }
}

enum OrderType {
  market;

  static OrderType fromApi(bridge.OrderType orderType) {
    if (orderType is bridge.OrderType_Market) {
      return OrderType.market;
    }

    throw Exception("Only market orders are supported! Received unexpected order type $orderType");
  }
}

class Order {
  final Leverage leverage;
  final double quantity;
  final ContractSymbol contractSymbol;
  final Direction direction;
  final OrderState status;
  final OrderType type;

  Order(
      {required this.leverage,
      required this.quantity,
      required this.contractSymbol,
      required this.direction,
      required this.status,
      required this.type});
}
