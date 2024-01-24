import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/wallet/wallet_change_notifier.dart';
import 'package:provider/provider.dart';
import 'package:qr_flutter/qr_flutter.dart';

class ReceiveScreen extends StatefulWidget {
  const ReceiveScreen({super.key});

  @override
  State<ReceiveScreen> createState() => _ReceiveScreenState();
}

class _ReceiveScreenState extends State<ReceiveScreen> {
  String? address;

  @override
  void initState() {
    super.initState();
    try {
      context
          .read<WalletChangeNotifier>()
          .service
          .getNewAddress()
          .then((a) => setState(() => address = a));
    } catch (e) {
      final messenger = ScaffoldMessenger.of(context);
      showSnackBar(messenger, e.toString());
    }
  }

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(25),
      child: Column(
        mainAxisAlignment: MainAxisAlignment.start,
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          address == null
              ? const SizedBox.square(
                  dimension: 350, child: Center(child: CircularProgressIndicator()))
              : SizedBox.square(
                  dimension: 350,
                  child: QrImageView(
                    data: address!,
                    eyeStyle: const QrEyeStyle(
                      eyeShape: QrEyeShape.square,
                      color: Colors.black,
                    ),
                    dataModuleStyle: const QrDataModuleStyle(
                      dataModuleShape: QrDataModuleShape.square,
                      color: Colors.black,
                    ),
                    embeddedImage: const AssetImage('assets/10101_logo_icon_white_background.png'),
                    embeddedImageStyle: const QrEmbeddedImageStyle(
                      size: Size(50, 50),
                    ),
                    version: QrVersions.auto,
                    padding: const EdgeInsets.all(5),
                  ),
                ),
          const SizedBox(height: 20),
          Container(
              padding: const EdgeInsets.all(15),
              decoration: BoxDecoration(
                  border: Border.all(color: tenTenOnePurple),
                  borderRadius: BorderRadius.circular(8)),
              child: address == null
                  ? const Center(child: CircularProgressIndicator())
                  : Row(mainAxisAlignment: MainAxisAlignment.spaceBetween, children: [
                      Text(address!),
                      GestureDetector(
                          child: const Icon(Icons.copy, size: 20),
                          onTap: () async {
                            Clipboard.setData(ClipboardData(text: address ?? "")).then((_) {
                              showSnackBar(
                                  ScaffoldMessenger.of(context), "Address copied to clipboard");
                            });
                          })
                    ]))
        ],
      ),
    );
  }
}
