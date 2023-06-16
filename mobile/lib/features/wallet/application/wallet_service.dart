import 'package:f_logs/f_logs.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/ffi.dart' as rust;
import 'package:get_10101/features/wallet/domain/lightning_invoice.dart';

class WalletService {
  const WalletService();

  Future<void> refreshWalletInfo() async {
    try {
      await rust.api.refreshWalletInfo();
    } catch (error) {
      FLog.error(text: "Failed to refresh wallet info: $error");
    }
  }

  Future<String?> createInvoice(Amount? amount) async {
    try {
      String invoice;
      if (amount != null) {
        invoice = await rust.api.createInvoiceWithAmount(amountSats: amount.sats);
      } else {
        invoice = await rust.api.createInvoiceWithoutAmount();
      }

      FLog.info(text: "Successfully created invoice.");
      return invoice;
    } catch (error) {
      FLog.error(text: "Error: $error", exception: error);
      return null;
    }
  }

  Future<LightningInvoice?> decodeInvoice(String invoice) async {
    try {
      FLog.debug(text: "Decoding invoice $invoice");
      rust.LightningInvoice lightningInvoice = await rust.api.decodeInvoice(invoice: invoice);
      FLog.debug(text: "Successfully decoded invoice.");
      return LightningInvoice.fromApi(lightningInvoice);
    } catch (error) {
      FLog.debug(text: "Failed to decode invoice: $error", exception: error);
      return null;
    }
  }

  Future<void> payInvoice(String invoice) async {
    await rust.api.sendPayment(invoice: invoice);
  }

  String getUnusedAddress() {
    return rust.api.getUnusedAddress();
  }
}
