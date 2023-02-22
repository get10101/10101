import 'package:f_logs/f_logs.dart';
import 'package:get_10101/features/wallet/domain/wallet_info.dart';
import 'package:get_10101/ffi.dart' as rust;

class WalletService {
  const WalletService();

   Future<WalletInfo?> getWalletInfo() async {
     try {
       final walletInfo = WalletInfo.fromApi(await rust.api.refreshWalletInfo());
       FLog.trace(text: 'Successfully retrieved wallet info');
       return walletInfo;
     } catch (error) {
       FLog.error(text: "Failed to get wallet info: $error");
       return null;
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

   Future<String?> createInvoice() async {
     try {
       String invoice = await rust.api.createInvoice();
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
