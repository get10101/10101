import 'package:f_logs/f_logs.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/ffi.dart' as rust;

class WalletService {
  const WalletService();

  Future<void> refreshWalletInfo() async {
    try {
      await rust.api.refreshWalletInfo();
    } catch (error) {
      FLog.error(text: "Failed to refresh wallet info: $error");
    }
  }

  Future<void> openChannel() async {
    try {
      await rust.api.openChannel();
      FLog.info(text: "Open Channel successfully started.");
    } catch (error) {
      FLog.error(text: "Error: $error", exception: error);
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

  String getNewAddress() {
    return rust.api.getNewAddress();
  }
}
