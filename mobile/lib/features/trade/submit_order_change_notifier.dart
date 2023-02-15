import 'package:flutter/material.dart';
import 'package:get_10101/features/trade/application/order_service.dart';
import 'package:get_10101/features/trade/domain/contract_symbol.dart';
import 'domain/trade_values.dart';

enum PendingOrderState {
  submitting,
  submittedSuccessfully,
  submissionFailed,
}

class PendingOrder {
  final TradeValues _tradeValues;
  PendingOrderState state = PendingOrderState.submitting;
  String? pendingOrderError;

  PendingOrder(this._tradeValues);
}

class SubmitOrderChangeNotifier extends ChangeNotifier {
  final OrderService orderService = OrderService();

  PendingOrder? _pendingOrder;

  submitPendingOrder(TradeValues tradeValues) async {
    _pendingOrder = PendingOrder(tradeValues);

    notifyListeners();

    await orderService.submitMarketOrder(
        tradeValues.leverage, tradeValues.quantity, ContractSymbol.btcusd, tradeValues.direction);
    _pendingOrder!.state = PendingOrderState.submittedSuccessfully;

    notifyListeners();
  }

  PendingOrder? get pendingOrder => _pendingOrder;

  TradeValues? get pendingOrderValues => _pendingOrder?._tradeValues;
}
