import 'package:flutter/material.dart';
import 'package:qr_flutter/qr_flutter.dart';

class CustomQrCode extends StatelessWidget {
  final String data;
  final String embeddedImagePath;

  const CustomQrCode({
    Key? key,
    required this.data,
    required this.embeddedImagePath,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return SizedBox.square(
      dimension: 350,
      child: QrImageView(
        data: data,
        eyeStyle: const QrEyeStyle(
          eyeShape: QrEyeShape.square,
          color: Colors.black,
        ),
        dataModuleStyle: const QrDataModuleStyle(
          dataModuleShape: QrDataModuleShape.square,
          color: Colors.black,
        ),
        embeddedImage: AssetImage(embeddedImagePath),
        embeddedImageStyle: const QrEmbeddedImageStyle(
          size: Size(50, 50),
        ),
        version: QrVersions.auto,
        padding: const EdgeInsets.all(5),
      ),
    );
  }
}
