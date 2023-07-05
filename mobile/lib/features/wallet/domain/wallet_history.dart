import 'package:get_10101/common/domain/model.dart';
import 'payment_flow.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as rust;

enum WalletHistoryItemDataType { lightning, onChain, trade, orderMatchingFee }

enum WalletHistoryStatus { pending, confirmed }

class WalletHistoryItemData {
  final PaymentFlow flow;
  final Amount amount;
  final WalletHistoryItemDataType type;
  final WalletHistoryStatus status;
  final DateTime timestamp;

  // on-chain
  final String? txid;

  // lightning
  final String? paymentHash;

  // trade
  final String? orderId;

  const WalletHistoryItemData(
      {required this.flow,
      required this.amount,
      required this.type,
      required this.status,
      required this.timestamp,
      this.paymentHash,
      this.orderId,
      this.txid});

  static WalletHistoryItemData fromApi(rust.WalletHistoryItem item) {
    PaymentFlow flow =
        item.flow == rust.PaymentFlow.Outbound ? PaymentFlow.outbound : PaymentFlow.inbound;
    Amount amount = Amount(item.amountSats);
    WalletHistoryStatus status = item.status == rust.Status.Pending
        ? WalletHistoryStatus.pending
        : WalletHistoryStatus.confirmed;

    DateTime timestamp = DateTime.fromMillisecondsSinceEpoch(item.timestamp * 1000);

    if (item.walletType is rust.WalletType_OnChain) {
      rust.WalletType_OnChain type = item.walletType as rust.WalletType_OnChain;

      return WalletHistoryItemData(
          flow: flow,
          amount: amount,
          status: status,
          type: WalletHistoryItemDataType.onChain,
          timestamp: timestamp,
          txid: type.txid);
    }

    if (item.walletType is rust.WalletType_Trade) {
      rust.WalletType_Trade type = item.walletType as rust.WalletType_Trade;

      return WalletHistoryItemData(
          flow: flow,
          amount: amount,
          status: status,
          type: WalletHistoryItemDataType.trade,
          timestamp: timestamp,
          orderId: type.orderId);
    }

    if (item.walletType is rust.WalletType_OrderMatchingFee) {
      rust.WalletType_OrderMatchingFee type = item.walletType as rust.WalletType_OrderMatchingFee;

      return WalletHistoryItemData(
          flow: flow,
          amount: amount,
          status: status,
          type: WalletHistoryItemDataType.orderMatchingFee,
          timestamp: timestamp,
          orderId: type.orderId);
    }

    return WalletHistoryItemData(
        flow: flow,
        amount: amount,
        status: status,
        type: WalletHistoryItemDataType.lightning,
        timestamp: timestamp,
        paymentHash: (item.walletType as rust.WalletType_Lightning).paymentHash);
  }
}
