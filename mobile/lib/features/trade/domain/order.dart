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
  late String id;
  final Leverage leverage;
  final double quantity;
  final ContractSymbol contractSymbol;
  final Direction direction;
  final OrderState state;
  final OrderType type;
  final double? executionPrice;
  final DateTime creationTimestamp;

  Order(
      {required this.id,
      required this.leverage,
      required this.quantity,
      required this.contractSymbol,
      required this.direction,
      required this.state,
      required this.type,
      required this.creationTimestamp,
      this.executionPrice});

  static Order fromApi(bridge.Order order) {
    return Order(
        id: order.id,
        leverage: Leverage(order.leverage),
        quantity: order.quantity,
        contractSymbol: ContractSymbol.fromApi(order.contractSymbol),
        direction: Direction.fromApi(order.direction),
        state: OrderState.fromApi(order.state),
        type: OrderType.fromApi(order.orderType),
        executionPrice: order.executionPrice,
        creationTimestamp: DateTime.fromMillisecondsSinceEpoch(order.creationTimestamp * 1000));
  }

  static bridge.Order apiDummy() {
    return const bridge.Order(
        id: "",
        leverage: 0,
        quantity: 0,
        contractSymbol: bridge.ContractSymbol.BtcUsd,
        direction: bridge.Direction.Long,
        orderType: bridge.OrderType.market(),
        state: bridge.OrderState.Open,
        creationTimestamp: 0,
        orderExpiryTimestamp: 0);
  }
}
