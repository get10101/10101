import 'package:get_10101/bridge_generated/bridge_definitions.dart' as rust;
import 'package:get_10101/common/domain/model.dart';

class LightningInvoice {
  final String description;
  final Amount amountSats;
  final DateTime timestamp;
  final String payee;
  final DateTime expiry;

  LightningInvoice(this.description, this.amountSats, this.timestamp, this.payee, this.expiry);

  static fromApi(rust.LightningInvoice invoice) {
    return LightningInvoice(
        invoice.description,
        Amount(invoice.amountSats),
        DateTime.fromMillisecondsSinceEpoch(invoice.timestamp * 1000),
        invoice.payee,
        DateTime.fromMillisecondsSinceEpoch(invoice.expiry * 1000));
  }
}
