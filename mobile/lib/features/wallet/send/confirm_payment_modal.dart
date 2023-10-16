import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/features/wallet/application/util.dart';
import 'package:get_10101/features/wallet/domain/destination.dart';
import 'package:get_10101/features/wallet/domain/wallet_type.dart';
import 'package:get_10101/features/wallet/send/payment_sent_change_notifier.dart';
import 'package:get_10101/features/wallet/send/send_dialog.dart';
import 'package:get_10101/features/wallet/wallet_change_notifier.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:go_router/go_router.dart';
import 'package:provider/provider.dart';
import 'package:slide_to_confirm/slide_to_confirm.dart';

void showConfirmPaymentModal(BuildContext context, Destination destination, Amount? amount) {
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
                height: 320,
                child: Scaffold(
                    body: ConfirmPayment(
                  destination: destination,
                  amount: amount,
                ))));
      });
}

class ConfirmPayment extends StatelessWidget {
  final Destination destination;
  final Amount? amount;

  const ConfirmPayment({super.key, required this.destination, this.amount});

  @override
  Widget build(BuildContext context) {
    final walletService = context.read<WalletChangeNotifier>().service;

    final amt = destination.amount.sats > 0 ? destination.amount : amount!;

    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.only(left: 20.0, top: 35.0, right: 20.0),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            const Text("Destination:", style: TextStyle(fontWeight: FontWeight.bold, fontSize: 16)),
            const SizedBox(height: 2),
            Text(truncateWithEllipsis(32, destination.raw), style: const TextStyle(fontSize: 16)),
            const SizedBox(height: 20),
            const Text("Payee:", style: TextStyle(fontWeight: FontWeight.bold, fontSize: 16)),
            const SizedBox(height: 2),
            Text(truncateWithEllipsis(32, destination.payee), style: const TextStyle(fontSize: 16)),
            const SizedBox(height: 20),
            const Text("Amount:", style: TextStyle(fontWeight: FontWeight.bold, fontSize: 16)),
            const SizedBox(height: 2),
            Text(amt.toString(), style: const TextStyle(fontSize: 16)),
            const SizedBox(height: 25),
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
                  context.read<PaymentChangeNotifier>().waitForPayment();
                  GoRouter.of(context).pop();
                  final messenger = ScaffoldMessenger.of(context);
                  if (destination.getWalletType() == WalletType.lightning) {
                    showDialog(
                        context: context,
                        useRootNavigator: true,
                        barrierDismissible: false, // Prevent user from leaving
                        builder: (BuildContext context) {
                          return SendDialog(destination: destination, amount: amt);
                        });
                  }

                  walletService.sendPayment(destination, amt).then((value) {
                    if (destination.getWalletType() == WalletType.onChain) {
                      GoRouter.of(context).pop();
                    }
                  }).catchError((error) {
                    logger.e("Failed to send payment: $error");
                    if (destination.getWalletType() == WalletType.onChain) {
                      showSnackBar(messenger, error.toString());
                    }
                    context.read<PaymentChangeNotifier>().failPayment();
                  });
                })
          ],
        ),
      ),
    );
  }
}
