import 'package:flutter/material.dart';
import 'package:qr_flutter/qr_flutter.dart';

class CustomQrCode extends StatelessWidget {
  final String data;
  final ImageProvider embeddedImage;
  final double embeddedImageSizeWidth;
  final double embeddedImageSizeHeight;
  final double dimension;

  const CustomQrCode({
    Key? key,
    required this.data,
    required this.embeddedImage,
    this.dimension = 350.0,
    this.embeddedImageSizeHeight = 50,
    this.embeddedImageSizeWidth = 50,
  }) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return SizedBox.square(
      dimension: dimension,
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
        embeddedImage: embeddedImage,
        embeddedImageStyle: QrEmbeddedImageStyle(
          size: Size(embeddedImageSizeHeight, embeddedImageSizeWidth),
        ),
        version: QrVersions.auto,
        padding: const EdgeInsets.all(5),
      ),
    );
  }
}
