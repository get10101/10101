import 'dart:developer';
import 'package:f_logs/model/flog/flog.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/application/order_service.dart';
import 'package:get_10101/features/trade/application/trade_values_service.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/domain/position.dart';
import 'domain/order.dart';
import 'domain/trade_values.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;

enum PendingOrderState {
  submitting,
  submittedSuccessfully,
  submissionFailed,
  orderFilled,
  orderFailed,
}

enum PositionAction {
  close,
  open,
}

class PendingOrder {
  final TradeValues _tradeValues;
  PendingOrderState state = PendingOrderState.submitting;
  String? pendingOrderError;
  final PositionAction positionAction;
  String? id;

  PendingOrder(this._tradeValues, this.positionAction);
}

class SubmitOrderChangeNotifier extends ChangeNotifier implements Subscriber {
  final OrderService orderService;
  PendingOrder? _pendingOrder;

  SubmitOrderChangeNotifier(this.orderService);

  submitPendingOrder(TradeValues tradeValues, PositionAction positionAction) async {
    _pendingOrder = PendingOrder(tradeValues, positionAction);

    // notify listeners about pending order in state "pending"
    notifyListeners();

    try {
      assert(tradeValues.quantity != null, 'Quantity cannot be null when submitting order');
      _pendingOrder!.id = await orderService.submitMarketOrder(tradeValues.leverage,
          tradeValues.quantity!, ContractSymbol.btcusd, tradeValues.direction);
      _pendingOrder!.state = PendingOrderState.submittedSuccessfully;
    } catch (exception) {
      FLog.error(text: "Failed to submit order: $exception");
      _pendingOrder!.state = PendingOrderState.submissionFailed;
    }

    // notify listeners about the status change of the pending order after submission
    notifyListeners();
  }

  // TODO: This is not optimal, because we map the Order in the change notifier. We can do this, but it would be better to do this on the service level.
  @override
  void notify(bridge.Event event) {
    log("Receiving this in the submit order change notifier: ${event.toString()}");

    if (event is bridge.Event_OrderUpdateNotification) {
      Order order = Order.fromApi(event.field0);

      if (_pendingOrder?.id == order.id) {
        switch (order.state) {
          case OrderState.open:
            return;
          case OrderState.filled:
            _pendingOrder!.state = PendingOrderState.orderFilled;
            break;
          case OrderState.failed:
            _pendingOrder!.state = PendingOrderState.orderFailed;
            break;
        }

        notifyListeners();
      }
    } else {
      log("Received unexpected event: ${event.toString()}");
    }
  }

  Future<void> closePosition(Position position) async {
    await submitPendingOrder(
        TradeValues(
            direction: position.direction.opposite(),
            margin: position.collateral,
            quantity: position.quantity,
            leverage: position.leverage,
            price: 0,
            liquidationPrice: position.liquidationPrice,
            fee: Amount.zero(),
            fundingRate: 0,
            tradeValuesService: TradeValuesService()),
        PositionAction.close);
  }

  PendingOrder? get pendingOrder => _pendingOrder;

  TradeValues? get pendingOrderValues => _pendingOrder?._tradeValues;
}
