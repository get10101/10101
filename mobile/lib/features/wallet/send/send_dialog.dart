import 'package:flutter/material.dart';
import 'package:get_10101/common/domain/background_task.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/common/task_status_dialog.dart';
import 'package:get_10101/common/value_data_row.dart';
import 'package:get_10101/features/wallet/domain/destination.dart';
import 'package:get_10101/features/wallet/send/payment_sent_change_notifier.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:provider/provider.dart';

class SendDialog extends StatelessWidget {
  final Destination destination;
  final Amount amount;

  const SendDialog({super.key, required this.destination, required this.amount});

  @override
  Widget build(BuildContext context) {
    final paymentChangeNotifier = context.watch<PaymentChangeNotifier>();

    final content = ValueDataRow(type: ValueType.amount, value: amount, label: "Amount");

    switch (paymentChangeNotifier.getPaymentStatus()) {
      case PaymentStatus.pending:
        return TaskStatusDialog(
            title: "Sending payment", status: TaskStatus.pending, content: content);
      case PaymentStatus.failed:
        return TaskStatusDialog(
            title: "Sending payment", status: TaskStatus.failed, content: content);
      case PaymentStatus.success:
        return TaskStatusDialog(
            title: "Sending payment",
            status: TaskStatus.success,
            content: content,
            navigateToRoute: WalletScreen.route);
    }
  }
}
