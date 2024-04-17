import 'dart:math';

import 'package:flutter_rust_bridge/flutter_rust_bridge.dart';
import 'package:get_10101/features/trade/domain/channel_opening_params.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/application/order_service.dart';
import 'package:get_10101/features/trade/application/trade_values_service.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/domain/position.dart';
import 'package:get_10101/features/trade/domain/order.dart';
import 'package:get_10101/features/trade/domain/trade_values.dart';

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
  Amount? pnl;
  FailureReason? failureReason;
  String? submitOrderError;

  PendingOrder(this._tradeValues, this.positionAction, this.pnl);

  TradeValues? get tradeValues => _tradeValues;
}

class SubmitOrderChangeNotifier extends ChangeNotifier implements Subscriber {
  final OrderService orderService;
  PendingOrder? _pendingOrder;

  SubmitOrderChangeNotifier(this.orderService);

  submitPendingOrder(TradeValues tradeValues, PositionAction positionAction,
      {ChannelOpeningParams? channelOpeningParams, Amount? pnl, bool stable = false}) async {
    _pendingOrder = PendingOrder(tradeValues, positionAction, pnl);

    // notify listeners about pending order in state "pending"
    notifyListeners();

    try {
      if (channelOpeningParams != null) {
        // TODO(holzeis): The coordinator leverage should not be hard coded here.
        final coordinatorCollateral = tradeValues.calculateMargin(Leverage(2.0));

        final coordinatorReserve =
            max(0, channelOpeningParams.coordinatorCollateral.sub(coordinatorCollateral).sats);
        final traderReserve =
            max(0, channelOpeningParams.traderCollateral.sub(tradeValues.margin!).sats);

        _pendingOrder!.id = await orderService.submitChannelOpeningMarketOrder(
            tradeValues.leverage,
            tradeValues.contracts,
            ContractSymbol.btcusd,
            tradeValues.direction,
            stable,
            Amount(coordinatorReserve),
            Amount(traderReserve));
      } else {
        _pendingOrder!.id = await orderService.submitMarketOrder(tradeValues.leverage,
            tradeValues.contracts, ContractSymbol.btcusd, tradeValues.direction, stable);
      }

      _pendingOrder!.state = PendingOrderState.submittedSuccessfully;
    } on FfiException catch (exception) {
      logger.e("Failed to submit order: $exception");
      _pendingOrder!.state = PendingOrderState.submissionFailed;
      _pendingOrder!.submitOrderError = exception.message;
    }

    // notify listeners about the status change of the pending order after submission
    notifyListeners();
  }

  @override
  void notify(bridge.Event event) {
    if (event is bridge.Event_OrderUpdateNotification) {
      Order order = Order.fromApi(event.field0);

      if (pendingOrder != null) {
        switch (order.state) {
          case OrderState.open:
          case OrderState.filling:
            return;
          case OrderState.filled:
            _pendingOrder!.state = PendingOrderState.orderFilled;
            break;
          case OrderState.failed:
          case OrderState.rejected:
            _pendingOrder!.state = PendingOrderState.orderFailed;
            break;
        }
        _pendingOrder!.failureReason = order.failureReason;

        notifyListeners();
      }
    } else {
      logger.w("Received unexpected event: ${event.toString()}");
    }
  }

  Future<void> closePosition(Position position, double? closingPrice, Amount? fee,
      {bool stable = false}) async {
    final tradeValues = TradeValues(
        direction: position.direction.opposite(),
        margin: position.collateral,
        quantity: position.quantity,
        leverage: position.leverage,
        price: closingPrice,
        liquidationPrice: position.liquidationPrice,
        fee: fee,
        expiry: position.expiry,
        tradeValuesService: const TradeValuesService());
    tradeValues.contracts = position.quantity;
    await submitPendingOrder(tradeValues, PositionAction.close,
        pnl: position.unrealizedPnl, stable: stable);
  }

  PendingOrder? get pendingOrder => _pendingOrder;

  TradeValues? get pendingOrderValues => _pendingOrder?._tradeValues;
}
