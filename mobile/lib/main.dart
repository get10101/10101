import 'package:flutter/material.dart';
import 'ffi.dart';

void main() {
  runApp(const MyApp());
}

class MyApp extends StatelessWidget {
  const MyApp({Key? key}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: '10101',
      theme: ThemeData(
        primarySwatch: Colors.blue,
      ),
      home: const TenTenOneApp(title: '10101'),
    );
  }
}

class TenTenOneApp extends StatefulWidget {
  const TenTenOneApp({Key? key, required this.title}) : super(key: key);

  final String title;

  @override
  State<TenTenOneApp> createState() => _TenTenOneAppState();
}

class _TenTenOneAppState extends State<TenTenOneApp> {

  @override
  void initState() {
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text(widget.title),
      ),
      body: Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: const <Widget>[
            Text("Welcome to 10101"),
          ],
        ),
      ),
    );
  }
}
