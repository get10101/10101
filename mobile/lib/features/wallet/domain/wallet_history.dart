import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/wallet/wallet_history_item.dart';
import 'payment_flow.dart';
import 'package:get_10101/bridge_generated/bridge_definitions.dart' as rust;

enum WalletHistoryStatus { pending, expired, confirmed, failed }

abstract class WalletHistoryItemData {
  final PaymentFlow flow;
  final Amount amount;
  final WalletHistoryStatus status;
  final DateTime timestamp;

  const WalletHistoryItemData(
      {required this.flow, required this.amount, required this.status, required this.timestamp});

  WalletHistoryItem toWidget();

  static WalletHistoryItemData fromApi(rust.WalletHistoryItem item) {
    PaymentFlow flow =
        item.flow == rust.PaymentFlow.Outbound ? PaymentFlow.outbound : PaymentFlow.inbound;
    Amount amount = Amount(item.amountSats);
    WalletHistoryStatus status = switch (item.status) {
      rust.Status.Pending => WalletHistoryStatus.pending,
      rust.Status.Expired => WalletHistoryStatus.expired,
      rust.Status.Confirmed => WalletHistoryStatus.confirmed,
      rust.Status.Failed => WalletHistoryStatus.failed,
    };

    DateTime timestamp = DateTime.fromMillisecondsSinceEpoch(item.timestamp * 1000);

    if (item.walletType is rust.WalletHistoryItemType_OnChain) {
      rust.WalletHistoryItemType_OnChain type =
          item.walletType as rust.WalletHistoryItemType_OnChain;

      return OnChainPaymentData(
        flow: flow,
        amount: amount,
        status: status,
        timestamp: timestamp,
        txid: type.txid,
        confirmations: type.confirmations,
        fee: type.feeSats != null ? Amount(type.feeSats!) : null,
      );
    }

    if (item.walletType is rust.WalletHistoryItemType_Trade) {
      rust.WalletHistoryItemType_Trade type = item.walletType as rust.WalletHistoryItemType_Trade;

      return TradeData(
          flow: flow, amount: amount, status: status, timestamp: timestamp, orderId: type.orderId);
    }

    if (item.walletType is rust.WalletHistoryItemType_OrderMatchingFee) {
      rust.WalletHistoryItemType_OrderMatchingFee type =
          item.walletType as rust.WalletHistoryItemType_OrderMatchingFee;

      return OrderMatchingFeeData(
          flow: flow,
          amount: amount,
          status: status,
          timestamp: timestamp,
          orderId: type.orderId,
          paymentHash: type.paymentHash);
    }

    if (item.walletType is rust.WalletHistoryItemType_JitChannelFee) {
      rust.WalletHistoryItemType_JitChannelFee type =
          item.walletType as rust.WalletHistoryItemType_JitChannelFee;

      return JitChannelOpenFeeData(
        flow: flow,
        amount: amount,
        status: status,
        timestamp: timestamp,
        txid: type.fundingTxid,
      );
    }

    rust.WalletHistoryItemType_Lightning type =
        item.walletType as rust.WalletHistoryItemType_Lightning;

    DateTime? expiry = type.expiryTimestamp != null
        ? DateTime.fromMillisecondsSinceEpoch(type.expiryTimestamp! * 1000)
        : null;

    return LightningPaymentData(
        flow: flow,
        amount: amount,
        status: status,
        timestamp: timestamp,
        preimage: type.paymentPreimage,
        description: type.description,
        paymentHash: type.paymentHash,
        feeMsats: type.feeMsat,
        expiry: expiry,
        invoice: type.invoice);
  }
}

class LightningPaymentData extends WalletHistoryItemData {
  final String paymentHash;
  final String? preimage;
  final String description;
  final String? invoice;
  final DateTime? expiry;
  final int? feeMsats;

  LightningPaymentData(
      {required super.flow,
      required super.amount,
      required super.status,
      required super.timestamp,
      required this.preimage,
      required this.description,
      required this.invoice,
      required this.expiry,
      required this.feeMsats,
      required this.paymentHash});

  @override
  WalletHistoryItem toWidget() {
    return LightningPaymentHistoryItem(data: this);
  }
}

class OnChainPaymentData extends WalletHistoryItemData {
  final String txid;
  final int confirmations;
  final Amount? fee;

  OnChainPaymentData(
      {required super.flow,
      required super.amount,
      required super.status,
      required super.timestamp,
      required this.confirmations,
      required this.fee,
      required this.txid});

  @override
  WalletHistoryItem toWidget() {
    return OnChainPaymentHistoryItem(data: this);
  }
}

class OrderMatchingFeeData extends WalletHistoryItemData {
  final String orderId;
  final String paymentHash;

  OrderMatchingFeeData(
      {required super.flow,
      required super.amount,
      required super.status,
      required super.timestamp,
      required this.orderId,
      required this.paymentHash});

  @override
  WalletHistoryItem toWidget() {
    return OrderMatchingFeeHistoryItem(data: this);
  }
}

class JitChannelOpenFeeData extends WalletHistoryItemData {
  final String txid;

  JitChannelOpenFeeData(
      {required super.flow,
      required super.amount,
      required super.status,
      required super.timestamp,
      required this.txid});

  @override
  WalletHistoryItem toWidget() {
    return JitChannelOpenFeeHistoryItem(data: this);
  }
}

class TradeData extends WalletHistoryItemData {
  final String orderId;

  TradeData(
      {required super.flow,
      required super.amount,
      required super.status,
      required super.timestamp,
      required this.orderId});

  @override
  WalletHistoryItem toWidget() {
    return TradeHistoryItem(data: this);
  }
}
