import 'package:get_10101/bridge_generated/bridge_definitions.dart' as rust;
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/wallet/domain/wallet_type.dart';

abstract class Destination {
  final Amount amount;
  final String description;
  final String payee;
  final String raw;

  Destination({required this.amount, this.description = "", this.payee = "", required this.raw});

  WalletType getWalletType();
}

class OnChainAddress extends Destination {
  final String address;

  OnChainAddress(
      {required super.amount,
      super.description = "",
      super.payee = "",
      required this.address,
      required super.raw});

  static fromAddress(rust.Destination_OnChainAddress address) {
    return OnChainAddress(
        amount: Amount.zero(), address: address.field0, payee: address.field0, raw: address.field0);
  }

  static fromApi(rust.Destination_Bip21 uri) {
    return OnChainAddress(
        amount: uri.amountSats != null ? Amount(uri.amountSats!) : Amount.zero(),
        description: uri.message,
        payee: uri.label.isNotEmpty ? uri.label : uri.address,
        address: uri.address,
        raw: uri.address);
  }

  @override
  WalletType getWalletType() {
    return WalletType.onChain;
  }
}

class LightningInvoice extends Destination {
  final DateTime timestamp;
  final DateTime expiry;

  LightningInvoice(
      {required super.amount,
      required super.description,
      required super.payee,
      required super.raw,
      required this.timestamp,
      required this.expiry});

  static fromApi(rust.Destination_Bolt11 invoice, String raw) {
    return LightningInvoice(
      description: invoice.description,
      amount: Amount(invoice.amountSats),
      raw: raw,
      timestamp: DateTime.fromMillisecondsSinceEpoch(invoice.timestamp * 1000),
      payee: invoice.payee,
      expiry: DateTime.fromMillisecondsSinceEpoch(invoice.expiry * 1000),
    );
  }

  @override
  WalletType getWalletType() {
    return WalletType.lightning;
  }
}
