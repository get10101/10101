import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';

enum OrderReason {
  manual,
  expired,
  liquidated;

  static OrderReason fromApi(bridge.OrderReason orderReason) {
    switch (orderReason) {
      case bridge.OrderReason.Manual:
        return OrderReason.manual;
      case bridge.OrderReason.Expired:
        return OrderReason.expired;
      case bridge.OrderReason.Liquidated:
        return OrderReason.liquidated;
    }
  }

  static bridge.OrderReason apiDummy() {
    return bridge.OrderReason.Manual;
  }
}

enum OrderState {
  open,
  filling,
  filled,
  failed,
  rejected;

  static OrderState fromApi(bridge.OrderState orderState) {
    switch (orderState) {
      case bridge.OrderState.Open:
        return OrderState.open;
      case bridge.OrderState.Filling:
        return OrderState.filling;
      case bridge.OrderState.Filled:
        return OrderState.filled;
      case bridge.OrderState.Failed:
        return OrderState.failed;
      case bridge.OrderState.Rejected:
        return OrderState.rejected;
    }
  }
}

enum FailureReasonType {
  protocolError,
  failed,
  timeout,
  rejected,
  unknown;
}

class FailureReason {
  final String? details;
  final FailureReasonType failureType;

  const FailureReason._({this.details, required this.failureType});

  static const FailureReason standardProtocolError = FailureReason._(
      failureType: FailureReasonType.protocolError, details: "Failed executing the DLC protocol.");
  static const FailureReason failed = FailureReason._(
      failureType: FailureReasonType.failed, details: "We failed processing the order.");
  static const FailureReason timeout = FailureReason._(
      failureType: FailureReasonType.timeout,
      details: "The order timed out before finding a match");
  static const FailureReason unknown = FailureReason._(
      failureType: FailureReasonType.unknown, details: "An unknown error occurred.");

  static FailureReason? fromApi(bridge.FailureReason? failureReason) {
    if (failureReason == null) {
      return null;
    }
    switch (failureReason) {
      case bridge.FailureReason_FailedToSetToFilling():
      case bridge.FailureReason_TradeRequest():
      case bridge.FailureReason_NodeAccess():
      case bridge.FailureReason_NoUsableChannel():
      case bridge.FailureReason_CollabRevert():
      case bridge.FailureReason_OrderNotAcceptable():
      case bridge.FailureReason_InvalidDlcOffer():
        return standardProtocolError;
      case bridge.FailureReason_TradeResponse():
        return FailureReason._(
            failureType: FailureReasonType.protocolError, details: failureReason.field0);
      case bridge.FailureReason_TimedOut():
        return timeout;
      case bridge.FailureReason_OrderRejected():
        return FailureReason._(
            failureType: FailureReasonType.rejected, details: failureReason.field0);
      case bridge.FailureReason_Unknown():
        return unknown;
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
  final Usd quantity;
  final ContractSymbol contractSymbol;
  final Direction direction;
  final OrderState state;
  final OrderType type;
  final Usd? executionPrice;
  final DateTime creationTimestamp;
  final OrderReason reason;
  final FailureReason? failureReason;

  Order(
      {required this.id,
      required this.leverage,
      required this.quantity,
      required this.contractSymbol,
      required this.direction,
      required this.state,
      required this.type,
      required this.creationTimestamp,
      this.executionPrice,
      required this.reason,
      required this.failureReason});

  static Order fromApi(bridge.Order order) {
    return Order(
        id: order.id,
        leverage: Leverage(order.leverage),
        quantity: Usd(order.quantity.ceil()),
        contractSymbol: ContractSymbol.fromApi(order.contractSymbol),
        direction: Direction.fromApi(order.direction),
        state: OrderState.fromApi(order.state),
        type: OrderType.fromApi(order.orderType),
        executionPrice: order.executionPrice != null ? Usd.fromDouble(order.executionPrice!) : null,
        creationTimestamp: DateTime.fromMillisecondsSinceEpoch(order.creationTimestamp * 1000),
        reason: OrderReason.fromApi(order.reason),
        failureReason: FailureReason.fromApi(order.failureReason));
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
        orderExpiryTimestamp: 0,
        reason: bridge.OrderReason.Manual);
  }
}
