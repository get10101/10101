import 'package:f_logs/model/flog/flog.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/trade/application/order_service.dart';
import 'package:get_10101/features/trade/application/trade_values_service.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'package:get_10101/features/trade/domain/position.dart';
import 'domain/trade_values.dart';

enum PendingOrderState {
  submitting,
  submittedSuccessfully,
  submissionFailed,
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

  PendingOrder(this._tradeValues, this.positionAction);
}

class SubmitOrderChangeNotifier extends ChangeNotifier {
  final OrderService orderService;
  PendingOrder? _pendingOrder;

  SubmitOrderChangeNotifier(this.orderService);

  submitPendingOrder(TradeValues tradeValues, PositionAction positionAction) async {
    _pendingOrder = PendingOrder(tradeValues, positionAction);

    // notify listeners about pending order in state "pending"
    notifyListeners();

    try {
      await orderService.submitMarketOrder(
          tradeValues.leverage, tradeValues.quantity, ContractSymbol.btcusd, tradeValues.direction);
      _pendingOrder!.state = PendingOrderState.submittedSuccessfully;
    } catch (exception) {
      FLog.error(text: "Failed to submit order: $exception");
      _pendingOrder!.state = PendingOrderState.submissionFailed;
    }

    // notify listeners about the status change of the pending order after submission
    notifyListeners();
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
