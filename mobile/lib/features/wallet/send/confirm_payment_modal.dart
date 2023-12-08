import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/features/trade/submit_order_change_notifier.dart';
import 'package:get_10101/features/trade/trade_value_change_notifier.dart';
import 'package:get_10101/features/wallet/application/util.dart';
import 'package:get_10101/features/wallet/domain/destination.dart';
import 'package:get_10101/features/wallet/domain/wallet_type.dart';
import 'package:get_10101/features/wallet/send/execute_payment_modal.dart';
import 'package:get_10101/features/wallet/send/payment_sent_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:go_router/go_router.dart';
import 'package:intl/intl.dart';
import 'package:provider/provider.dart';
import 'package:slide_to_confirm/slide_to_confirm.dart';

void showConfirmPaymentModal(
    BuildContext context, Destination destination, bool payWithUsdp, Amount sats, Amount usdp) {
  showModalBottomSheet<void>(
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(
          top: Radius.circular(20),
        ),
      ),
      clipBehavior: Clip.antiAlias,
      isScrollControlled: true,
      useRootNavigator: false,
      context: context,
      builder: (BuildContext context) {
        return SingleChildScrollView(
            child: SizedBox(
                height: 420,
                child: Scaffold(
                    body: ConfirmPayment(
                  payWithUsdp: payWithUsdp,
                  destination: destination,
                  sats: sats,
                  usdp: usdp,
                ))));
      });
}

class ConfirmPayment extends StatelessWidget {
  final Destination destination;
  final bool payWithUsdp;
  final Amount sats;
  final Amount usdp;

  const ConfirmPayment(
      {super.key,
      required this.destination,
      required this.payWithUsdp,
      required this.sats,
      required this.usdp});

  @override
  Widget build(BuildContext context) {
    final walletService = context.read<WalletChangeNotifier>().service;
    final submitOderChangeNotifier = context.read<SubmitOrderChangeNotifier>();
    final formatter = NumberFormat("#,###,##0.00", "en");

    final tradeValuesChangeNotifier = context.watch<TradeValuesChangeNotifier>();

    final tradeValues = tradeValuesChangeNotifier.fromDirection(Direction.long);
    tradeValues.updateLeverage(Leverage(1));

    Amount amt = destination.amount;
    if (destination.amount.sats == 0) {
      if (payWithUsdp) {
        // if the destination does not specify an amount and we ar paying with the usdp balance we
        // calculate the amount from the quantity point of view.
        tradeValues.updateQuantity(usdp);
        amt = tradeValues.margin!;
      } else {
        // Otherwise it is a regular lightning payment and we just pay the given amount.
        amt = sats;
      }
    } else {
      // if the amount is set on the invoice we need to pay the amount no matter what. That might
      // lead to the usdp amount to jump by one dollar depending on the current bid price
      tradeValues.updateMargin(destination.amount);
    }

    return SafeArea(
      child: Container(
        color: Colors.white,
        padding: const EdgeInsets.only(left: 20.0, top: 35.0, right: 20.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const Text("Summary", style: TextStyle(fontSize: 20)),
                const SizedBox(height: 10),
                Container(
                  padding: const EdgeInsets.all(20),
                  decoration: BoxDecoration(
                      color: tenTenOnePurple.shade200.withOpacity(0.1),
                      borderRadius: BorderRadius.circular(8)),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      const Text("Amount", style: TextStyle(color: Colors.grey, fontSize: 16)),
                      const SizedBox(height: 5),
                      Visibility(
                          visible: payWithUsdp,
                          replacement: Text(amt.sats == 0 ? "Max" : amt.toString(),
                              style: const TextStyle(fontSize: 16)),
                          child: Row(
                            mainAxisAlignment: MainAxisAlignment.spaceBetween,
                            children: [
                              Text("~ \$ ${formatter.format(tradeValues.quantity?.toInt ?? 0)}",
                                  style: const TextStyle(fontSize: 16)),
                              Text(amt.toString(),
                                  style: const TextStyle(fontSize: 16, color: Colors.grey))
                            ],
                          )),
                      const Divider(height: 40, indent: 0, endIndent: 0),
                      const Text("Destination", style: TextStyle(color: Colors.grey, fontSize: 16)),
                      const SizedBox(height: 5),
                      Text(truncateWithEllipsis(26, destination.raw),
                          style: const TextStyle(fontSize: 16)),
                      const Divider(height: 40, indent: 0, endIndent: 0),
                      const Text("Payee", style: TextStyle(color: Colors.grey, fontSize: 16)),
                      const SizedBox(height: 5),
                      Text(truncateWithEllipsis(26, destination.payee),
                          style: const TextStyle(fontSize: 16)),
                      const SizedBox(height: 10),
                    ],
                  ),
                )
              ],
            ),
            const SizedBox(height: 15),
            ConfirmationSlider(
                text: "Swipe to confirm",
                textStyle: const TextStyle(color: Colors.black87),
                height: 40,
                foregroundColor: tenTenOnePurple,
                sliderButtonContent: const Icon(
                  Icons.chevron_right,
                  color: Colors.white,
                  size: 20,
                ),
                onConfirmation: () async {
                  GoRouter.of(context).pop();
                  final messenger = ScaffoldMessenger.of(context);
                  if (destination.getWalletType() == WalletType.lightning) {
                    context.read<PaymentChangeNotifier>().waitForPayment();
                    if (payWithUsdp) {
                      submitOderChangeNotifier.submitPendingOrder(tradeValues, PositionAction.open);
                    }
                    showExecuteUsdpPaymentModal(context, destination, amt, payWithUsdp);
                  } else {
                    walletService.sendPayment(destination, amt).then((value) {
                      GoRouter.of(context).pop();
                    }).catchError((error) {
                      logger.e("Failed to send payment: $error");
                      showSnackBar(messenger, error.toString());
                    });
                  }
                })
          ],
        ),
      ),
    );
  }
}
