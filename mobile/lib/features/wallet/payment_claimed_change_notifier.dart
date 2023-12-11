import 'package:flutter/material.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/application/event_service.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/dummy_values.dart';
import 'package:get_10101/common/global_keys.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/features/swap/swap_trade_values.dart';
import 'package:get_10101/features/swap/swap_trade_values_service.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/features/trade/domain/price.dart';
import 'package:get_10101/features/trade/domain/trade_values.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:get_10101/features/wallet/receive/receive_usdp_dialog.dart';
import 'package:get_10101/ffi.dart' as rust;
import 'package:get_10101/logger/logger.dart';
import 'package:provider/provider.dart';

class PaymentClaimedChangeNotifier extends ChangeNotifier implements Subscriber {
  bool _claimed = false;
  late final SwapTradeValuesService tradeValuesService;

  late final SwapTradeValues _sellTradeValues;

  void waitForPayment() => _claimed = false;

  bool isClaimed() => _claimed;

  PaymentClaimedChangeNotifier(this.tradeValuesService) {
    _sellTradeValues = _initDummyOrder();
  }

  SwapTradeValues _initDummyOrder() {
    return SwapTradeValues.fromQuantity(
        quantity: Amount(10),
        leverage: Leverage(1),
        price: null,
        fundingRate: fundingRateSell,
        direction: Direction.short,
        tradeValuesService: tradeValuesService);
  }

  @override
  void notify(bridge.Event event) {
    if (event is bridge.Event_PaymentClaimed) {
      final paymentAmountMsats = event.field0;
      final paymentHash = event.field1;
      final paymentAmountSats = paymentAmountMsats / 1000;

      logger.i("Amount : $paymentAmountSats hash: $paymentHash");

      if (rust.api.isUsdpPayment(paymentHash: paymentHash)) {
        // TODO: how do I do this properly? According to stackoverflow one should not open a dialog from within a change notifier
        logger.i("Received payment which should be converted to USDP");
        var context = shellNavigatorKey.currentContext!;
        final submitOrderChangeNotifier = context.read<SubmitOrderChangeNotifier>();
        final margin = Amount(paymentAmountSats.ceil());

        if (_sellTradeValues.price == null) {
          logger.e(
              "Could not convert received payments directly into USDP because we do not have a price");
          _claimed = true;
          final messenger = ScaffoldMessenger.of(context);
          showSnackBar(messenger,
              "Could not convert received payment into USDP. Please try todo it manually.");
          notifyListeners();
          return;
        }

        final price = _sellTradeValues.price!;
        var fee = _sellTradeValues.fee;

        final quantity = Amount(((paymentAmountSats / 100000000) * price).ceil());
        logger.i("Posting order for quantity $quantity");

        // TODO: why do we need these values when submitting a new order? :/
        var liquidationPrice = 100000.0;
        var fundingRate = 100000.0;
        var expiry = DateTime.now();

        submitOrderChangeNotifier.submitPendingOrder(
            TradeValues(
                direction: Direction.short,
                margin: margin,
                quantity: quantity,
                leverage: Leverage(1.0),
                price: price,
                liquidationPrice: liquidationPrice,
                fee: fee,
                fundingRate: fundingRate,
                expiry: expiry,
                tradeValuesService: tradeValuesService),
            PositionAction.open,
            stable: true);

        _claimed = true;
        showDialog(
            context: context,
            useRootNavigator: true,
            barrierDismissible: false, // Prevent user from leaving
            builder: (BuildContext context) {
              return const ReceiveUsdpDialog();
            });
      } else {
        logger.i("Received normal payment");
        _claimed = true;
        super.notifyListeners();
      }
    } else if (event is bridge.Event_PriceUpdateNotification) {
      logger.i("Received new price");
      updatePrice(Price.fromApi(event.field0));
    }
  }

  void updatePrice(Price price) {
    if (price.bid != _sellTradeValues.price) {
      _sellTradeValues.updatePriceAndMargin(price.bid);
    }
  }
}
